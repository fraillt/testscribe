use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::time::Instant;

use futures::FutureExt;

use crate::processor::filter::Filter;
use crate::processor::logger::{Logger, PanicLocation, SkipReason, TestRunInfo, TestStatusUpdate};
use crate::processor::panic::extract_string_from_panic_payload;
use crate::report::TestReport;
use crate::test_case::{CloneFns, EnvFns, FqFnName, TestCase, TestFn, Value};

pub struct EnvData {
    name: FqFnName<'static>,
    data: Value,
    is_empty: bool,
}

impl EnvData {
    fn new_empty() -> Self {
        EnvData {
            name: FqFnName::new("", "NoEnvironment"),
            data: Value::new(()),
            is_empty: true,
        }
    }

    async fn init_specific_env(self, info: &EnvFns) -> Result<Self, SkipReason> {
        let name = (info.self_type)();
        if name != self.name {
            let env_res = AssertUnwindSafe((info.create_env)(self.data))
                .catch_unwind()
                .await;
            match env_res {
                Ok(data) => Ok(EnvData {
                    name,
                    data,
                    is_empty: false,
                }),
                Err(err) => {
                    let message = extract_string_from_panic_payload(&err)
                        .unwrap_or_else(|| "<unknown msg>".to_string());
                    Err(SkipReason::Panicked {
                        name,
                        location: PanicLocation::Environment,
                        message,
                    })
                }
            }
        } else {
            Ok(self)
        }
    }

    fn empty_env(self) -> Self {
        if self.is_empty {
            self
        } else {
            EnvData::new_empty()
        }
    }
}

pub enum RunState {
    Run { test_data: Value, env: EnvData },
    Skip(SkipReason),
}

impl RunState {
    pub fn init() -> Self {
        RunState::Run {
            env: EnvData::new_empty(),
            test_data: Value::new(()),
        }
    }

    pub fn clone_state(&self, clone_fns: &CloneFns) -> RunState {
        match self {
            RunState::Run { test_data, env } => RunState::Run {
                test_data: (clone_fns.state)(test_data),
                env: EnvData {
                    name: env.name,
                    data: ((clone_fns.env)(&env.data)),
                    is_empty: env.is_empty,
                },
            },
            RunState::Skip(skip_reason) => RunState::Skip(skip_reason.clone()),
        }
    }

    pub async fn run_test(
        self,
        filter: &dyn Filter,
        logger: &mut dyn Logger,
        test: &'static TestCase,
        started_at: Instant,
        info: TestRunInfo,
        test_fn: &TestFn,
        env_info: &Option<EnvFns>,
        test_params: Option<Value>,
    ) -> RunState {
        let name = test.name;
        match self {
            RunState::Run { test_data, env } => {
                if !filter.should_run(test, &info) {
                    logger.log(
                        test,
                        TestStatusUpdate::Skipped {
                            info,
                            reason: SkipReason::Ignored { name },
                        },
                        started_at.elapsed(),
                    );
                    return RunState::Skip(SkipReason::Ignored { name });
                }
                execute_test(
                    logger,
                    test,
                    started_at,
                    info,
                    test_fn,
                    test_data,
                    env,
                    env_info,
                    test_params,
                )
                .await
            }
            RunState::Skip(reason) => {
                logger.log(
                    test,
                    TestStatusUpdate::Skipped {
                        info,
                        reason: reason.clone(),
                    },
                    started_at.elapsed(),
                );
                RunState::Skip(reason)
            }
        }
    }
}

async fn execute_test(
    logger: &mut dyn Logger,
    test: &'static TestCase,
    started_at: Instant,
    info: TestRunInfo,
    test_fn: &TestFn,
    parent_data: Value,
    env: EnvData,
    env_info: &Option<EnvFns>,
    params: Option<Value>,
) -> RunState {
    let mut env = if let Some(required_env) = env_info {
        match env.init_specific_env(required_env).await {
            Ok(env) => env,
            Err(reason) => return RunState::Skip(reason),
        }
    } else {
        env.empty_env()
    };

    let name = test.name;
    logger.log(
        test,
        TestStatusUpdate::Started { info },
        started_at.elapsed(),
    );
    let logger = unsafe {
        // SAFETY:
        // I expect that no one will take this out of this function
        // TODO: I could probably enforce it by passing in reference
        let logger = std::mem::transmute::<&mut dyn Logger, &'static mut dyn Logger>(logger);
        Rc::new(RefCell::new(logger))
    };
    let test_res = AssertUnwindSafe(test_fn.invoke(
        TestReport::new(test, logger.clone(), started_at),
        parent_data,
        &mut env.data,
        params.unwrap_or_else(|| Value::new(())),
    ))
    .catch_unwind()
    .await;
    let Ok(mut logger) = Rc::try_unwrap(logger) else {
        panic!("Expectations object MUST NOT outlive a test");
    };
    match test_res {
        Ok(test_data) => {
            logger.get_mut().log(
                test,
                TestStatusUpdate::Finished {
                    panic_message: None,
                },
                started_at.elapsed(),
            );
            RunState::Run { test_data, env }
        }
        Err(err) => {
            let message = extract_string_from_panic_payload(&err)
                .unwrap_or_else(|| "<unknown msg>".to_string());
            logger.get_mut().log(
                test,
                TestStatusUpdate::Finished {
                    panic_message: Some(message.clone()),
                },
                started_at.elapsed(),
            );
            RunState::Skip(SkipReason::Panicked {
                name,
                location: PanicLocation::Test,
                message,
            })
        }
    }
}
