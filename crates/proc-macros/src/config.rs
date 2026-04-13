use syn::parse::{Error, Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, Ident, Meta, Path, Token};

#[derive(Debug)]
pub struct StandaloneConfig {
    /// The `standalone` path itself, kept for error spans.
    pub path: Path,
    /// Attributes to put on the generated test-harness wrapper, e.g. `tokio::test`.
    /// Empty means the default `#[test]` (only valid for sync test functions).
    pub harness: Vec<Meta>,
}

#[derive(Debug)]
pub struct TestConfig {
    pub standalone: Option<StandaloneConfig>,
    pub is_cloneable: Option<Path>,
    pub is_cloneable_async: Option<Path>,
    pub tags: Vec<Ident>,
}

#[derive(Debug)]
pub enum Config {
    Params,
    Test(TestConfig),
}

impl Parse for Config {
    fn parse(input: ParseStream) -> Result<Self> {
        let list = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        let mut is_param_mode = false;
        let mut test_config = TestConfig {
            standalone: None,
            is_cloneable: None,
            is_cloneable_async: None,
            tags: Vec::new(),
        };
        for param in list {
            if param.path().is_ident("cloneable") {
                test_config.is_cloneable = Some(param.path().clone());
            } else if param.path().is_ident("cloneable_async") {
                test_config.is_cloneable_async = Some(param.path().clone());
            } else if param.path().is_ident("params") {
                is_param_mode = true;
            } else if param.path().is_ident("standalone") {
                test_config.standalone = Some(parse_standalone(param)?);
            } else if param.path().is_ident("tags") {
                let value = param.require_name_value()?;
                if let Expr::Array(list) = &value.value {
                    for tag in &list.elems {
                        if let Expr::Path(tag) = tag {
                            let Some(tag) = tag.path.get_ident() else {
                                return Err(Error::new(
                                    tag.span(),
                                    "Tags must be a simple ident, without path segments",
                                ));
                            };
                            test_config.tags.push(tag.clone());
                        } else {
                            return Err(Error::new(tag.span(), "Tags must be a simple idents"));
                        }
                    }
                } else {
                    return Err(Error::new(
                        value.span(),
                        "Tags must be an array e.g. [my_tag]",
                    ));
                }
            } else {
                return Err(Error::new(
                    param.span(),
                    "Unexpected name, possible values are: params, cloneable, cloneable_async, standalone, tags",
                ));
            }
        }
        if is_param_mode {
            if let Some(path) = test_config.is_cloneable {
                return Err(Error::new(
                    path.span(),
                    "`cloneable` cannot be used in `params` mode",
                ));
            }
            if let Some(path) = test_config.is_cloneable_async {
                return Err(Error::new(
                    path.span(),
                    "`cloneable_async` cannot be used in `params` mode",
                ));
            }
            if let Some(standalone) = test_config.standalone {
                return Err(Error::new(
                    standalone.path.span(),
                    "`standalone` cannot be used in `params` mode",
                ));
            }
            if let Some(path) = test_config.tags.first() {
                return Err(Error::new(
                    path.span(),
                    "`tags` cannot be used in `params` mode",
                ));
            }
            Ok(Config::Params)
        } else {
            if test_config.is_cloneable.is_some()
                && let Some(path) = &test_config.is_cloneable_async
            {
                return Err(Error::new(
                    path.span(),
                    "choose only one: `cloneable` or `cloneable_async`",
                ));
            }
            Ok(Config::Test(test_config))
        }
    }
}

fn parse_standalone(param: Meta) -> Result<StandaloneConfig> {
    match param {
        Meta::Path(path) => Ok(StandaloneConfig {
            path,
            harness: Vec::new(),
        }),
        Meta::List(list) => {
            let harness: Vec<Meta> = list
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?
                .into_iter()
                .collect();
            if harness.is_empty() {
                return Err(Error::new(
                    list.span(),
                    "expected a test harness attribute, e.g. `standalone(tokio::test)`",
                ));
            }
            Ok(StandaloneConfig {
                path: list.path,
                harness,
            })
        }
        Meta::NameValue(nv) => Err(Error::new(
            nv.span(),
            "`standalone` does not take a value; use `standalone` or `standalone(tokio::test)`",
        )),
    }
}
