//! The batches and the outcome of every test in them.
//!
//! A batch is what its tests said the last time each of them ran. The batches with a name are
//! stored in a SQLite database. The batch without one, [`DEFAULT_BATCH`], lives in a database
//! in memory and is gone with the server.
//!
//! A batch knows every test its test binaries announced, also the ones that have not run:
//! those are [`Status::NotRun`]. So a test that is missing from a batch is not in the code,
//! and a test that did not run is not mistaken for one that was deleted.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Result, params};
use serde::{Deserialize, Serialize};

use crate::outcome::{Status, TestOutcome, TestPath, Timing};

/// The batch of the runs that name none. It is never stored.
pub const DEFAULT_BATCH: &str = "default";

/// What "run again" in the UI executes, unless the batch says otherwise.
pub const DEFAULT_COMMAND: &str = "cargo test --no-fail-fast";

const SCHEMA: &str = "
    PRAGMA foreign_keys = ON;
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    CREATE TABLE IF NOT EXISTS batches (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        updated_at INTEGER NOT NULL,
        -- What to run the next time: 'all', 'selected' or 'new_and_failed'.
        mode TEXT NOT NULL DEFAULT 'all',
        -- The tests picked for 'selected': a JSON list of {binary, path}.
        selected TEXT NOT NULL DEFAULT '[]'
    );
    CREATE TABLE IF NOT EXISTS tests (
        batch INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
        -- The test binary, without the hash cargo adds to its file name.
        binary TEXT NOT NULL,
        -- The names from the root down to the test, as a JSON list.
        path TEXT NOT NULL,
        root TEXT NOT NULL,
        -- The place of the test in its tree, to list the tests in the order they are written.
        ord INTEGER NOT NULL,
        status TEXT NOT NULL,
        text TEXT NOT NULL,
        PRIMARY KEY (batch, binary, path)
    ) WITHOUT ROWID;
";

/// The columns added since the first version of the database, each with its table.
const ADDED_COLUMNS: &[(&str, &str, &str)] = &[
    // How often the test was executed in the run of its tree, and the time in the test itself
    // and in getting it going, in microseconds.
    ("tests", "runs", "INTEGER NOT NULL DEFAULT 0"),
    ("tests", "test_us", "INTEGER NOT NULL DEFAULT 0"),
    ("tests", "setup_us", "INTEGER NOT NULL DEFAULT 0"),
    // Where the test is in the sources: the file, from the workspace, and the line.
    ("tests", "file", "TEXT NOT NULL DEFAULT ''"),
    ("tests", "line", "INTEGER NOT NULL DEFAULT 0"),
    // Whether the test failed only when it was executed again for a test below it.
    ("tests", "failed_again", "INTEGER NOT NULL DEFAULT 0"),
    // What 'run again' executes, and where. Without a command, DEFAULT_COMMAND.
    ("batches", "command", "TEXT"),
    ("batches", "dir", "TEXT"),
    // The state of the sources when a test binary of the batch last reported.
    ("batches", "git_commit", "TEXT"),
    ("batches", "git_branch", "TEXT"),
    ("batches", "git_dirty", "INTEGER"),
];

/// What a batch runs the next time its tests ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Everything. A tree that runs replaces what the batch knew of it.
    All,
    /// The tests picked in the UI, and the tests above them.
    Selected,
    /// The tests that have not run yet, and the ones that did not pass.
    NewAndFailed,
}

impl RunMode {
    fn as_str(self) -> &'static str {
        match self {
            RunMode::All => "all",
            RunMode::Selected => "selected",
            RunMode::NewAndFailed => "new_and_failed",
        }
    }

    fn parse(text: &str) -> RunMode {
        match text {
            "selected" => RunMode::Selected,
            "new_and_failed" => RunMode::NewAndFailed,
            _ => RunMode::All,
        }
    }
}

/// A test of some test binary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestId {
    pub binary: String,
    pub path: TestPath,
}

/// A test as its test binary announces it, whether it runs or not.
#[derive(Debug, Clone)]
pub struct Announced {
    pub path: TestPath,
    /// The place of the test among the tests of its binary, in the order they are written.
    pub ord: usize,
    pub file: String,
    pub line: u32,
}

/// The state of the sources a batch was run on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Sources {
    pub commit: Option<String>,
    pub branch: Option<String>,
    /// Whether there were changes that are not committed.
    pub dirty: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub name: String,
    /// Whether the batch is in the database, or only in memory.
    pub stored: bool,
    pub mode: RunMode,
    /// Milliseconds since 1970.
    pub updated_at: i64,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub not_run: u32,
    #[serde(flatten)]
    pub sources: Sources,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredTest {
    pub binary: String,
    pub path: TestPath,
    pub status: Status,
    pub text: String,
    #[serde(flatten)]
    pub timing: Timing,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Serialize)]
pub struct Batch {
    pub name: String,
    pub stored: bool,
    pub mode: RunMode,
    pub selected: Vec<TestId>,
    /// What "run again" executes, and in which directory.
    pub command: String,
    pub dir: Option<String>,
    #[serde(flatten)]
    pub sources: Sources,
    /// By binary, and within a binary in the order the tests are written.
    pub tests: Vec<StoredTest>,
}

/// A batch as a file: what its tests said, and nothing that differs between two runs of the
/// same code, so that the file can be kept next to the code and compared with later runs.
#[derive(Debug, Serialize, Deserialize)]
pub struct Export {
    /// The version of this format.
    pub testscribe_batch: u32,
    pub tests: Vec<ExportedTest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedTest {
    pub binary: String,
    pub path: TestPath,
    pub status: Status,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Deleted,
    Modified,
    /// The older side has an outcome of the test, and the newer side does not, but the test
    /// may well be there: it did not run, or its test binary did not report at all.
    NotRun,
}

/// A line of an outcome, and which side of a comparison has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffLine {
    /// `"+"` for a line only the newer side has, `"-"` for one only the older side has, and
    /// `" "` for a line of both.
    pub side: &'static str,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Change {
    pub binary: String,
    pub path: TestPath,
    pub kind: ChangeKind,
    pub from: Option<Status>,
    pub to: Option<Status>,
    pub lines: Vec<DiffLine>,
}

/// How a test that ran on both sides of a comparison ran on each.
#[derive(Debug, Serialize)]
pub struct TimeChange {
    pub binary: String,
    pub path: TestPath,
    pub from: Timing,
    pub to: Timing,
}

#[derive(Debug, Default, Serialize)]
pub struct Diff {
    pub changes: Vec<Change>,
    pub times: Vec<TimeChange>,
}

pub struct Store {
    stored: Connection,
    memory: Connection,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

fn path_json(path: &TestPath) -> String {
    serde_json::to_string(path).expect("names are plain text")
}

fn prepare(connection: &Connection) -> Result<()> {
    connection.execute_batch(SCHEMA)?;
    for (table, column, declaration) in ADDED_COLUMNS {
        let there = connection
            .prepare(&format!(
                "SELECT 1 FROM pragma_table_info('{table}') WHERE name = '{column}'"
            ))?
            .exists([])?;
        if !there {
            connection.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {column} {declaration}"
            ))?;
        }
    }
    Ok(())
}

impl Store {
    /// Opens the database at `path`, creating it if need be. Without a path, the batches
    /// with a name are kept in memory too.
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let stored = match path {
            Some(path) => Connection::open(path)?,
            None => Connection::open_in_memory()?,
        };
        let memory = Connection::open_in_memory()?;
        prepare(&stored)?;
        prepare(&memory)?;
        Ok(Self { stored, memory })
    }

    fn connection(&self, batch: &str) -> &Connection {
        if batch == DEFAULT_BATCH {
            &self.memory
        } else {
            &self.stored
        }
    }

    fn connection_mut(&mut self, batch: &str) -> &mut Connection {
        if batch == DEFAULT_BATCH {
            &mut self.memory
        } else {
            &mut self.stored
        }
    }

    fn batch_id(&self, batch: &str) -> Result<Option<i64>> {
        self.connection(batch)
            .query_row("SELECT id FROM batches WHERE name = ?1", [batch], |row| {
                row.get(0)
            })
            .optional()
    }

    /// Makes sure the batch is there, and returns its id.
    fn touch(&self, batch: &str) -> Result<i64> {
        self.connection(batch).query_row(
            "INSERT INTO batches (name, updated_at) VALUES (?1, ?2)
             ON CONFLICT (name) DO UPDATE SET updated_at = excluded.updated_at
             RETURNING id",
            params![batch, now()],
            |row| row.get(0),
        )
    }

    /// A test binary of the batch reports, with all its tests. The tests the batch knows of
    /// this binary that are not among them are no longer in the code, and are forgotten.
    /// The ones that are new to the batch have not run yet.
    pub fn binary_reports(
        &mut self,
        batch: &str,
        binary: &str,
        announced: &[Announced],
        sources: &Sources,
        dir: Option<&str>,
    ) -> Result<()> {
        let id = self.touch(batch)?;
        let transaction = self.connection_mut(batch).transaction()?;
        transaction.execute(
            "UPDATE batches SET git_commit = ?2, git_branch = ?3, git_dirty = ?4,
                dir = coalesce(dir, ?5)
             WHERE id = ?1",
            params![id, sources.commit, sources.branch, sources.dirty, dir],
        )?;
        {
            let existing: HashSet<String> =
                announced.iter().map(|test| path_json(&test.path)).collect();
            let mut known =
                transaction.prepare("SELECT path FROM tests WHERE batch = ?1 AND binary = ?2")?;
            let known = known
                .query_map(params![id, binary], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>>>()?;
            for path in known.iter().filter(|path| !existing.contains(*path)) {
                transaction.execute(
                    "DELETE FROM tests WHERE batch = ?1 AND binary = ?2 AND path = ?3",
                    params![id, binary, path],
                )?;
            }
            let mut announce = transaction.prepare(
                "INSERT INTO tests (batch, binary, path, root, ord, status, text, file, line)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'not_run', '', ?6, ?7)
                 ON CONFLICT (batch, binary, path) DO UPDATE
                 SET ord = excluded.ord, file = excluded.file, line = excluded.line",
            )?;
            for test in announced {
                announce.execute(params![
                    id,
                    binary,
                    path_json(&test.path),
                    test.path.first().map_or("", String::as_str),
                    test.ord as i64,
                    test.file,
                    test.line,
                ])?;
            }
        }
        transaction.commit()
    }

    /// The whole of a tree is about to run: what the batch knew of its tests is forgotten.
    pub fn tree_starts_over(&self, batch: &str, binary: &str, root: &str) -> Result<()> {
        let id = self.touch(batch)?;
        self.connection(batch).execute(
            "UPDATE tests
             SET status = 'not_run', text = '', runs = 0, test_us = 0, setup_us = 0,
                 failed_again = 0
             WHERE batch = ?1 AND binary = ?2 AND root = ?3",
            params![id, binary, root],
        )?;
        Ok(())
    }

    /// Keeps the outcome of a test that ran, over what the batch knew of it.
    pub fn save_test(&self, batch: &str, binary: &str, outcome: &TestOutcome) -> Result<()> {
        let id = self.touch(batch)?;
        // A test its binary did not announce comes after the ones it did.
        self.connection(batch).execute(
            "INSERT INTO tests
                (batch, binary, path, root, ord, status, text, runs, test_us, setup_us,
                 failed_again)
             VALUES (?1, ?2, ?3, ?4, 1000000, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (batch, binary, path) DO UPDATE
             SET status = excluded.status, text = excluded.text, runs = excluded.runs,
                 test_us = excluded.test_us, setup_us = excluded.setup_us,
                 failed_again = excluded.failed_again",
            params![
                id,
                binary,
                path_json(&outcome.path),
                outcome.path.first().map_or("", String::as_str),
                outcome.status.as_str(),
                outcome.text,
                outcome.timing.runs,
                outcome.timing.test_us as i64,
                outcome.timing.setup_us as i64,
                outcome.failed_again,
            ],
        )?;
        Ok(())
    }

    /// How every test of a tree did the last time it ran, and whether it failed only when
    /// it was executed again.
    pub fn statuses(
        &self,
        batch: &str,
        binary: &str,
        root: &str,
    ) -> Result<HashMap<TestPath, (Status, bool)>> {
        let Some(id) = self.batch_id(batch)? else {
            return Ok(HashMap::new());
        };
        let mut tests = self.connection(batch).prepare(
            "SELECT path, status, failed_again FROM tests
             WHERE batch = ?1 AND binary = ?2 AND root = ?3",
        )?;
        let rows = tests.query_map(params![id, binary, root], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })?;
        let mut statuses = HashMap::new();
        for row in rows {
            let (path, status, failed_again) = row?;
            if let Ok(path) = serde_json::from_str(&path) {
                statuses.insert(path, (Status::parse(&status), failed_again));
            }
        }
        Ok(statuses)
    }

    /// What the batch runs the next time, and the tests picked for [`RunMode::Selected`].
    pub fn run(&self, batch: &str) -> Result<(RunMode, Vec<TestId>)> {
        let run = self
            .connection(batch)
            .query_row(
                "SELECT mode, selected FROM batches WHERE name = ?1",
                [batch],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        Ok(match run {
            Some((mode, selected)) => (
                RunMode::parse(&mode),
                serde_json::from_str(&selected).unwrap_or_default(),
            ),
            None => (RunMode::All, Vec::new()),
        })
    }

    /// Sets what the batch runs the next time. Without `selected`, the tests picked earlier
    /// stay picked.
    pub fn set_run(&self, batch: &str, mode: RunMode, selected: Option<&[TestId]>) -> Result<()> {
        let connection = self.connection(batch);
        connection.execute(
            "UPDATE batches SET mode = ?2 WHERE name = ?1",
            params![batch, mode.as_str()],
        )?;
        if let Some(selected) = selected {
            let selected = serde_json::to_string(selected).expect("names are plain text");
            connection.execute(
                "UPDATE batches SET selected = ?2 WHERE name = ?1",
                params![batch, selected],
            )?;
        }
        Ok(())
    }

    /// What "run again" executes for the batch, and in which directory.
    pub fn command(&self, batch: &str) -> Result<(String, Option<String>)> {
        let command = self
            .connection(batch)
            .query_row(
                "SELECT command, dir FROM batches WHERE name = ?1",
                [batch],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .optional()?;
        let (command, dir) = command.unwrap_or_default();
        Ok((command.unwrap_or_else(|| DEFAULT_COMMAND.to_string()), dir))
    }

    pub fn set_command(&self, batch: &str, command: &str, dir: &str) -> Result<()> {
        let id = self.touch(batch)?;
        let text = |text: &str| Some(text.trim().to_string()).filter(|text| !text.is_empty());
        self.connection(batch).execute(
            "UPDATE batches SET command = ?2, dir = ?3 WHERE id = ?1",
            params![id, text(command), text(dir)],
        )?;
        Ok(())
    }

    pub fn delete(&self, batch: &str) -> Result<()> {
        self.connection(batch)
            .execute("DELETE FROM batches WHERE name = ?1", [batch])?;
        Ok(())
    }

    /// Forgets a test binary of the batch: one that is no longer in the code never reports
    /// again, so nothing else would.
    pub fn delete_binary(&self, batch: &str, binary: &str) -> Result<()> {
        self.connection(batch).execute(
            "DELETE FROM tests WHERE binary = ?2
             AND batch = (SELECT id FROM batches WHERE name = ?1)",
            params![batch, binary],
        )?;
        Ok(())
    }

    /// Every batch: the one in memory first, then the stored ones, the latest first.
    pub fn batches(&self) -> Result<Vec<BatchSummary>> {
        let mut batches = Vec::new();
        for (connection, stored) in [(&self.memory, false), (&self.stored, true)] {
            let mut query = connection.prepare(
                "SELECT name, mode, updated_at,
                    (SELECT count(*) FROM tests WHERE batch = id AND status = 'passed'),
                    (SELECT count(*) FROM tests WHERE batch = id AND status = 'failed'),
                    (SELECT count(*) FROM tests WHERE batch = id AND status = 'skipped'),
                    (SELECT count(*) FROM tests WHERE batch = id AND status = 'not_run'),
                    git_commit, git_branch, git_dirty
                 FROM batches ORDER BY updated_at DESC, id DESC",
            )?;
            let rows = query.query_map([], |row| {
                Ok(BatchSummary {
                    name: row.get(0)?,
                    stored,
                    mode: RunMode::parse(&row.get::<_, String>(1)?),
                    updated_at: row.get(2)?,
                    passed: row.get(3)?,
                    failed: row.get(4)?,
                    skipped: row.get(5)?,
                    not_run: row.get(6)?,
                    sources: Sources {
                        commit: row.get(7)?,
                        branch: row.get(8)?,
                        dirty: row.get(9)?,
                    },
                })
            })?;
            for row in rows {
                batches.push(row?);
            }
        }
        Ok(batches)
    }

    fn tests(&self, batch: &str) -> Result<Option<Vec<StoredTest>>> {
        let Some(id) = self.batch_id(batch)? else {
            return Ok(None);
        };
        let mut query = self.connection(batch).prepare(
            "SELECT binary, path, status, text, runs, test_us, setup_us, file, line
             FROM tests WHERE batch = ?1
             ORDER BY binary, ord, path",
        )?;
        let rows = query.query_map([id], |row| {
            Ok(StoredTest {
                binary: row.get(0)?,
                path: serde_json::from_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                status: Status::parse(&row.get::<_, String>(2)?),
                text: row.get(3)?,
                timing: Timing {
                    runs: row.get(4)?,
                    test_us: row.get::<_, i64>(5)? as u64,
                    setup_us: row.get::<_, i64>(6)? as u64,
                },
                file: row.get(7)?,
                line: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>().map(Some)
    }

    /// The batch with all its tests, if there is such a batch.
    pub fn batch(&self, batch: &str) -> Result<Option<Batch>> {
        let Some(tests) = self.tests(batch)? else {
            return Ok(None);
        };
        let (mode, selected) = self.run(batch)?;
        let (command, dir) = self.command(batch)?;
        let sources = self.connection(batch).query_row(
            "SELECT git_commit, git_branch, git_dirty FROM batches WHERE name = ?1",
            [batch],
            |row| {
                Ok(Sources {
                    commit: row.get(0)?,
                    branch: row.get(1)?,
                    dirty: row.get(2)?,
                })
            },
        )?;
        Ok(Some(Batch {
            name: batch.to_string(),
            stored: batch != DEFAULT_BATCH,
            mode,
            selected,
            command,
            dir,
            sources,
            tests,
        }))
    }

    /// What is different in batch `to`, seen from batch `from`. `None` if one of the batches
    /// does not exist.
    pub fn diff(&self, from: &str, to: &str) -> Result<Option<Diff>> {
        let (Some(from), Some(to)) = (self.tests(from)?, self.tests(to)?) else {
            return Ok(None);
        };
        Ok(Some(diff_tests(from, to)))
    }

    /// The batch as a file, if there is such a batch: the tests that have an outcome.
    pub fn export(&self, batch: &str) -> Result<Option<Export>> {
        Ok(self.tests(batch)?.map(|tests| Export {
            testscribe_batch: 1,
            tests: tests
                .into_iter()
                .filter(|test| test.status != Status::NotRun)
                .map(|test| ExportedTest {
                    binary: test.binary,
                    path: test.path,
                    status: test.status,
                    text: test.text,
                })
                .collect(),
        }))
    }

    /// Makes the batch what the file says, whatever it was before.
    pub fn import(&mut self, batch: &str, export: Export) -> Result<()> {
        let id = self.touch(batch)?;
        let transaction = self.connection_mut(batch).transaction()?;
        transaction.execute("DELETE FROM tests WHERE batch = ?1", [id])?;
        {
            let mut insert = transaction.prepare(
                "INSERT OR REPLACE INTO tests (batch, binary, path, root, ord, status, text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (nr, test) in export.tests.iter().enumerate() {
                insert.execute(params![
                    id,
                    test.binary,
                    path_json(&test.path),
                    test.path.first().map_or("", String::as_str),
                    nr as i64,
                    test.status.as_str(),
                    test.text,
                ])?;
            }
        }
        transaction.commit()
    }
}

impl Export {
    /// The tests of the file, as the tests of a batch.
    pub fn into_tests(self) -> Vec<StoredTest> {
        let test = |test: ExportedTest| StoredTest {
            binary: test.binary,
            path: test.path,
            status: test.status,
            text: test.text,
            timing: Timing::default(),
            file: String::new(),
            line: 0,
        };
        self.tests.into_iter().map(test).collect()
    }
}

/// What is different in the tests `to`, seen from the tests `from`.
///
/// A test is known by its binary and its path. With an outcome only in `to` it is added, and
/// with outcomes that read differently it is modified. With an outcome only in `from`, it is
/// deleted if `to` knows its binary and not the test. If `to` knows the test and has not run
/// it, or does not know the binary at all, the test did not run: it may well be there.
pub fn diff_tests(from: Vec<StoredTest>, to: Vec<StoredTest>) -> Diff {
    let id = |test: &StoredTest| (test.binary.clone(), test.path.clone());
    let reported: HashSet<String> = to.iter().map(|test| test.binary.clone()).collect();
    let mut older: HashMap<_, _> = from.into_iter().map(|test| (id(&test), test)).collect();
    let mut diff = Diff::default();
    for test in to {
        let before = older
            .remove(&id(&test))
            .filter(|test| test.status != Status::NotRun);
        let kind = match &before {
            None if test.status == Status::NotRun => continue,
            None => ChangeKind::Added,
            Some(_) if test.status == Status::NotRun => ChangeKind::NotRun,
            Some(before) if before.text == test.text => {
                if before.timing.runs > 0 && test.timing.runs > 0 {
                    diff.times.push(TimeChange {
                        from: before.timing,
                        to: test.timing,
                        binary: test.binary,
                        path: test.path,
                    });
                }
                continue;
            }
            Some(_) => ChangeKind::Modified,
        };
        let before_text = before.as_ref().map_or("", |test| test.text.as_str());
        diff.changes.push(Change {
            kind,
            from: before.as_ref().map(|test| test.status),
            to: Some(test.status).filter(|status| *status != Status::NotRun),
            lines: diff_lines(before_text, &test.text),
            binary: test.binary,
            path: test.path,
        });
    }
    let gone = older
        .into_values()
        .filter(|test| test.status != Status::NotRun);
    diff.changes.extend(gone.map(|test| Change {
        kind: if reported.contains(&test.binary) {
            ChangeKind::Deleted
        } else {
            ChangeKind::NotRun
        },
        from: Some(test.status),
        to: None,
        lines: diff_lines(&test.text, ""),
        binary: test.binary,
        path: test.path,
    }));
    // A test comes right after the test above it: its path starts with that one's.
    diff.changes
        .sort_by(|a, b| (&a.binary, &a.path).cmp(&(&b.binary, &b.path)));
    // The tests that took the most time more or less come first.
    let change = |time: &TimeChange| {
        let total = |timing: Timing| (timing.test_us + timing.setup_us) as i64;
        (total(time.to) - total(time.from)).abs()
    };
    diff.times
        .sort_by_key(|time| std::cmp::Reverse(change(time)));
    diff
}

/// The lines of `from` and `to` in one list, each marked with the side that has it.
pub fn diff_lines(from: &str, to: &str) -> Vec<DiffLine> {
    let from: Vec<&str> = from.lines().collect();
    let to: Vec<&str> = to.lines().collect();
    // common[i][j]: how many lines the rests from[i..] and to[j..] have in common, in order.
    let mut common = vec![vec![0u32; to.len() + 1]; from.len() + 1];
    for i in (0..from.len()).rev() {
        for j in (0..to.len()).rev() {
            common[i][j] = if from[i] == to[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let line = |side, text: &str| DiffLine {
        side,
        text: text.to_string(),
    };
    let mut lines = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < from.len() && j < to.len() {
        if from[i] == to[j] {
            lines.push(line(" ", from[i]));
            i += 1;
            j += 1;
        } else if common[i + 1][j] >= common[i][j + 1] {
            lines.push(line("-", from[i]));
            i += 1;
        } else {
            lines.push(line("+", to[j]));
            j += 1;
        }
    }
    lines.extend(from[i..].iter().map(|text| line("-", text)));
    lines.extend(to[j..].iter().map(|text| line("+", text)));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINARY: &str = "orders";
    const ROOT: &str = "orders::OrderPlaced";

    fn path(names: &[&str]) -> TestPath {
        names.iter().map(|name| format!("orders::{name}")).collect()
    }

    fn outcome(names: &[&str], status: Status, text: &str) -> TestOutcome {
        TestOutcome {
            path: path(names),
            status,
            text: text.to_string(),
            timing: Timing {
                runs: 2,
                test_us: 300,
                setup_us: 40,
            },
            failed_again: false,
        }
    }

    /// The tree as its binary announces it: `OrderPlaced`, and below it the tests `below`.
    fn announce(store: &mut Store, batch: &str, below: &[&str]) {
        let mut announced = vec![path(&["OrderPlaced"])];
        announced.extend(below.iter().map(|name| path(&["OrderPlaced", name])));
        let announced: Vec<Announced> = announced
            .into_iter()
            .enumerate()
            .map(|(ord, path)| Announced {
                path,
                ord,
                file: "tests/orders.rs".to_string(),
                line: 10 * ord as u32,
            })
            .collect();
        store
            .binary_reports(
                batch,
                BINARY,
                &announced,
                &Sources::default(),
                Some("/work"),
            )
            .unwrap();
    }

    fn run(store: &mut Store, batch: &str, whole_tree: bool, outcomes: &[TestOutcome]) {
        if whole_tree {
            store.tree_starts_over(batch, BINARY, ROOT).unwrap();
        }
        for outcome in outcomes {
            store.save_test(batch, BINARY, outcome).unwrap();
        }
    }

    /// What the batch knows: the last name, the status and the text of every test, in order.
    fn known(store: &Store, batch: &str) -> Vec<(String, Status, String)> {
        let batch = store.batch(batch).unwrap().unwrap();
        batch
            .tests
            .into_iter()
            .map(|test| {
                let name = test.path.last().unwrap().trim_start_matches("orders::");
                (name.to_string(), test.status, test.text)
            })
            .collect()
    }

    fn told(tests: &[(&str, Status, &str)]) -> Vec<(String, Status, String)> {
        tests
            .iter()
            .map(|(name, status, text)| (name.to_string(), *status, text.to_string()))
            .collect()
    }

    fn whole_tree() -> Vec<TestOutcome> {
        vec![
            outcome(&["OrderPlaced"], Status::Passed, "placed"),
            outcome(&["OrderPlaced", "OrderPaid"], Status::Failed, "paid, wrong"),
            outcome(
                &["OrderPlaced", "OrderCancelled"],
                Status::Passed,
                "cancelled",
            ),
        ]
    }

    /// A batch with the whole tree announced and run.
    fn batch_with_whole_tree(store: &mut Store, batch: &str) {
        announce(store, batch, &["OrderPaid", "OrderCancelled"]);
        run(store, batch, true, &whole_tree());
    }

    #[test]
    fn test_that_is_announced_has_not_run_until_it_does() {
        let mut store = Store::open(None).unwrap();
        announce(&mut store, "work", &["OrderPaid"]);
        assert_eq!(
            known(&store, "work"),
            told(&[
                ("OrderPlaced", Status::NotRun, ""),
                ("OrderPaid", Status::NotRun, "")
            ])
        );
        let batch = store.batch("work").unwrap().unwrap();
        assert_eq!(
            (batch.tests[1].file.as_str(), batch.tests[1].line),
            ("tests/orders.rs", 10)
        );
        assert_eq!(batch.dir.as_deref(), Some("/work"));
    }

    #[test]
    fn part_of_a_tree_that_ran_leaves_the_rest_of_it_as_it_was() {
        let mut store = Store::open(None).unwrap();
        batch_with_whole_tree(&mut store, "work");

        let again = [
            outcome(&["OrderPlaced", "OrderPaid"], Status::Passed, "paid"),
            outcome(&["OrderPlaced"], Status::Passed, "placed again"),
        ];
        run(&mut store, "work", false, &again);

        assert_eq!(
            known(&store, "work"),
            told(&[
                ("OrderPlaced", Status::Passed, "placed again"),
                ("OrderPaid", Status::Passed, "paid"),
                ("OrderCancelled", Status::Passed, "cancelled"),
            ])
        );
    }

    #[test]
    fn whole_tree_that_runs_starts_over() {
        let mut store = Store::open(None).unwrap();
        batch_with_whole_tree(&mut store, "work");
        run(
            &mut store,
            "work",
            true,
            &[outcome(&["OrderPlaced"], Status::Passed, "placed")],
        );
        assert_eq!(
            known(&store, "work"),
            told(&[
                ("OrderPlaced", Status::Passed, "placed"),
                ("OrderPaid", Status::NotRun, ""),
                ("OrderCancelled", Status::NotRun, ""),
            ])
        );
    }

    #[test]
    fn test_that_is_no_longer_in_the_code_is_forgotten_when_its_binary_reports() {
        let mut store = Store::open(None).unwrap();
        batch_with_whole_tree(&mut store, "work");
        announce(&mut store, "work", &["OrderPaid", "OrderShipped"]);
        assert_eq!(
            known(&store, "work"),
            told(&[
                ("OrderPlaced", Status::Passed, "placed"),
                ("OrderPaid", Status::Failed, "paid, wrong"),
                ("OrderShipped", Status::NotRun, ""),
            ])
        );
        store.delete_binary("work", BINARY).unwrap();
        assert!(known(&store, "work").is_empty());
    }

    #[test]
    fn named_batches_are_stored_and_the_default_batch_is_not() {
        let dir = std::env::temp_dir().join(format!("testscribe-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("batches.sqlite");
        {
            let mut store = Store::open(Some(&file)).unwrap();
            batch_with_whole_tree(&mut store, "work");
            batch_with_whole_tree(&mut store, DEFAULT_BATCH);
            let names: Vec<_> = store
                .batches()
                .unwrap()
                .into_iter()
                .map(|batch| (batch.name, batch.stored))
                .collect();
            assert_eq!(
                names,
                vec![("default".to_string(), false), ("work".to_string(), true)]
            );
            store.set_run("work", RunMode::NewAndFailed, None).unwrap();
            store
                .set_command("work", "cargo test -p orders", "")
                .unwrap();
        }
        let store = Store::open(Some(&file)).unwrap();
        let batches = store.batches().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].name, "work");
        assert_eq!((batches[0].passed, batches[0].failed), (2, 1));
        assert_eq!(batches[0].mode, RunMode::NewAndFailed);
        let batch = store.batch("work").unwrap().unwrap();
        let timing = batch.tests[0].timing;
        assert_eq!((timing.runs, timing.test_us, timing.setup_us), (2, 300, 40));
        assert_eq!(
            (batch.command.as_str(), batch.dir),
            ("cargo test -p orders", None)
        );
        assert!(store.batch(DEFAULT_BATCH).unwrap().is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn kinds(diff: &Diff) -> Vec<(&str, ChangeKind)> {
        diff.changes
            .iter()
            .map(|change| (change.path.last().unwrap().as_str(), change.kind))
            .collect()
    }

    #[test]
    fn diff_tells_the_tests_that_were_added_deleted_and_modified() {
        let mut store = Store::open(None).unwrap();
        batch_with_whole_tree(&mut store, "before");
        announce(&mut store, "after", &["OrderPaid", "OrderShipped"]);
        run(
            &mut store,
            "after",
            true,
            &[
                outcome(&["OrderPlaced"], Status::Passed, "placed"),
                outcome(
                    &["OrderPlaced", "OrderPaid"],
                    Status::Passed,
                    "paid\nin full",
                ),
                outcome(&["OrderPlaced", "OrderShipped"], Status::Passed, "shipped"),
            ],
        );

        let diff = store.diff("before", "after").unwrap().unwrap();
        assert_eq!(
            kinds(&diff),
            vec![
                ("orders::OrderCancelled", ChangeKind::Deleted),
                ("orders::OrderPaid", ChangeKind::Modified),
                ("orders::OrderShipped", ChangeKind::Added),
            ]
        );
        let modified = &diff.changes[1];
        assert_eq!(
            (modified.from, modified.to),
            (Some(Status::Failed), Some(Status::Passed))
        );
        let lines: Vec<_> = modified
            .lines
            .iter()
            .map(|line| format!("{}{}", line.side, line.text))
            .collect();
        assert_eq!(lines, vec!["-paid, wrong", "+paid", "+in full"]);
        // The test that says the same on both sides is compared by its times.
        assert_eq!(diff.times.len(), 1);
        assert_eq!(diff.times[0].path, path(&["OrderPlaced"]));

        assert!(store.diff("before", "nowhere").unwrap().is_none());
    }

    #[test]
    fn test_that_did_not_run_is_not_told_as_deleted() {
        let mut store = Store::open(None).unwrap();
        batch_with_whole_tree(&mut store, "before");
        // The newer batch knows the tree, and only ran a part of it.
        announce(&mut store, "after", &["OrderPaid", "OrderCancelled"]);
        run(
            &mut store,
            "after",
            false,
            &[outcome(&["OrderPlaced"], Status::Passed, "placed")],
        );
        let diff = store.diff("before", "after").unwrap().unwrap();
        assert_eq!(
            kinds(&diff),
            vec![
                ("orders::OrderCancelled", ChangeKind::NotRun),
                ("orders::OrderPaid", ChangeKind::NotRun),
            ]
        );

        // A batch whose test binary never reported: the tests did not run there either.
        store.set_command("empty", "", "").unwrap();
        let diff = store.diff("before", "empty").unwrap().unwrap();
        assert!(
            diff.changes
                .iter()
                .all(|change| change.kind == ChangeKind::NotRun)
        );
        assert_eq!(diff.changes.len(), 3);
    }

    #[test]
    fn batch_goes_to_a_file_and_back_without_what_differs_between_runs() {
        let mut store = Store::open(None).unwrap();
        batch_with_whole_tree(&mut store, "work");
        announce(
            &mut store,
            "work",
            &["OrderPaid", "OrderCancelled", "OrderShipped"],
        );

        let export = store.export("work").unwrap().unwrap();
        let file = serde_json::to_string(&export).unwrap();
        assert!(!file.contains("test_us") && !file.contains("OrderShipped"));

        store
            .import("copy", serde_json::from_str(&file).unwrap())
            .unwrap();
        let diff = store.diff("work", "copy").unwrap().unwrap();
        assert!(diff.changes.is_empty());
        let same = serde_json::from_str::<Export>(&file).unwrap().into_tests();
        let diff = diff_tests(same, store.batch("work").unwrap().unwrap().tests);
        assert!(diff.changes.is_empty());
    }

    #[test]
    fn diff_of_lines_keeps_what_both_sides_have() {
        let lines: Vec<_> = diff_lines("a\nb\nc\nd", "a\nc\nx\nd")
            .into_iter()
            .map(|line| format!("{}{}", line.side, line.text))
            .collect();
        assert_eq!(lines, vec![" a", "-b", " c", "+x", " d"]);
    }
}
