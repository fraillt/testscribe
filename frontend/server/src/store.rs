//! The batches and the outcome of every test in them.
//!
//! A batch is what its tests said the last time each of them ran. The batches with a name are
//! stored in a SQLite database. The batch without one, [`DEFAULT_BATCH`], lives in a database
//! in memory and is gone with the server.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Result, params};
use serde::{Deserialize, Serialize};

use crate::outcome::{Status, TestOutcome, TestPath};

/// The batch of the runs that name none. It is never stored.
pub const DEFAULT_BATCH: &str = "default";

const SCHEMA: &str = "
    PRAGMA foreign_keys = ON;
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

/// What a batch runs the next time its tests ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Everything. A tree that runs replaces what the batch knew of it.
    All,
    /// The tests picked in the UI, and the tests above them.
    Selected,
    /// The tests the batch does not know yet, and the ones that did not pass.
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
}

#[derive(Debug, Serialize)]
pub struct StoredTest {
    pub binary: String,
    pub path: TestPath,
    pub status: Status,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Batch {
    pub name: String,
    pub stored: bool,
    pub mode: RunMode,
    pub selected: Vec<TestId>,
    /// By binary, and within a binary in the order the tests are written.
    pub tests: Vec<StoredTest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Deleted,
    Modified,
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

impl Store {
    /// Opens the database at `path`, creating it if need be. Without a path, the batches
    /// with a name are kept in memory too.
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let stored = match path {
            Some(path) => Connection::open(path)?,
            None => Connection::open_in_memory()?,
        };
        let memory = Connection::open_in_memory()?;
        stored.execute_batch(SCHEMA)?;
        memory.execute_batch(SCHEMA)?;
        Ok(Self { stored, memory })
    }

    fn connection(&self, batch: &str) -> &Connection {
        if batch == DEFAULT_BATCH {
            &self.memory
        } else {
            &self.stored
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

    /// A test binary of the batch reports. The tests the batch knows of this binary that
    /// are not among `existing` are no longer in the code, and are forgotten.
    pub fn binary_reports(
        &self,
        batch: &str,
        binary: &str,
        existing: &HashSet<TestPath>,
    ) -> Result<()> {
        let id = self.touch(batch)?;
        let connection = self.connection(batch);
        let mut known =
            connection.prepare("SELECT path FROM tests WHERE batch = ?1 AND binary = ?2")?;
        let known = known
            .query_map(params![id, binary], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>>>()?;
        for path in known {
            let gone = serde_json::from_str::<TestPath>(&path)
                .map_or(true, |path| !existing.contains(&path));
            if gone {
                connection.execute(
                    "DELETE FROM tests WHERE batch = ?1 AND binary = ?2 AND path = ?3",
                    params![id, binary, path],
                )?;
            }
        }
        Ok(())
    }

    /// Keeps the outcome of the tests of a tree that ran, over what the batch knew of them.
    /// What it knew of the other tests of the tree stays, unless `whole_tree` ran: then the
    /// outcomes are all there is to know of the tree.
    pub fn save_tree(
        &mut self,
        batch: &str,
        binary: &str,
        root: &str,
        whole_tree: bool,
        outcomes: &[TestOutcome],
        place: &HashMap<TestPath, usize>,
    ) -> Result<()> {
        let id = self.touch(batch)?;
        let connection = if batch == DEFAULT_BATCH {
            &mut self.memory
        } else {
            &mut self.stored
        };
        let transaction = connection.transaction()?;
        if whole_tree {
            transaction.execute(
                "DELETE FROM tests WHERE batch = ?1 AND binary = ?2 AND root = ?3",
                params![id, binary, root],
            )?;
        }
        {
            let mut save = transaction.prepare(
                "INSERT INTO tests (batch, binary, path, root, ord, status, text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT (batch, binary, path) DO UPDATE
                 SET ord = excluded.ord, status = excluded.status, text = excluded.text",
            )?;
            for (nr, outcome) in outcomes.iter().enumerate() {
                // A test the tree did not announce comes after the ones it did.
                let ord = place
                    .get(&outcome.path)
                    .copied()
                    .unwrap_or(place.len() + nr);
                save.execute(params![
                    id,
                    binary,
                    path_json(&outcome.path),
                    root,
                    ord as i64,
                    outcome.status.as_str(),
                    outcome.text,
                ])?;
            }
        }
        transaction.commit()
    }

    /// How every test of a tree did the last time it ran, for the tests the batch knows.
    pub fn statuses(
        &self,
        batch: &str,
        binary: &str,
        root: &str,
    ) -> Result<HashMap<TestPath, Status>> {
        let Some(id) = self.batch_id(batch)? else {
            return Ok(HashMap::new());
        };
        let mut tests = self.connection(batch).prepare(
            "SELECT path, status FROM tests WHERE batch = ?1 AND binary = ?2 AND root = ?3",
        )?;
        let rows = tests.query_map(params![id, binary, root], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut statuses = HashMap::new();
        for row in rows {
            let (path, status) = row?;
            if let Ok(path) = serde_json::from_str(&path) {
                statuses.insert(path, Status::parse(&status));
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

    pub fn delete(&self, batch: &str) -> Result<()> {
        self.connection(batch)
            .execute("DELETE FROM batches WHERE name = ?1", [batch])?;
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
                    (SELECT count(*) FROM tests WHERE batch = id AND status = 'skipped')
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
            "SELECT binary, path, status, text FROM tests WHERE batch = ?1
             ORDER BY binary, root, ord, path",
        )?;
        let rows = query.query_map([id], |row| {
            Ok(StoredTest {
                binary: row.get(0)?,
                path: serde_json::from_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                status: Status::parse(&row.get::<_, String>(2)?),
                text: row.get(3)?,
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
        Ok(Some(Batch {
            name: batch.to_string(),
            stored: batch != DEFAULT_BATCH,
            mode,
            selected,
            tests,
        }))
    }

    /// What is different in batch `to`, seen from batch `from`: the tests only `to` has are
    /// added, the ones only `from` has are deleted, and the ones that read differently are
    /// modified. `None` if one of the batches does not exist.
    pub fn diff(&self, from: &str, to: &str) -> Result<Option<Vec<Change>>> {
        let (Some(from), Some(to)) = (self.tests(from)?, self.tests(to)?) else {
            return Ok(None);
        };
        Ok(Some(diff_tests(from, to)))
    }
}

fn diff_tests(from: Vec<StoredTest>, to: Vec<StoredTest>) -> Vec<Change> {
    let id = |test: &StoredTest| (test.binary.clone(), test.path.clone());
    let mut older: HashMap<_, _> = from.into_iter().map(|test| (id(&test), test)).collect();
    let mut changes = Vec::new();
    for test in to {
        match older.remove(&id(&test)) {
            Some(before) if before.text == test.text => {}
            Some(before) => changes.push(Change {
                kind: ChangeKind::Modified,
                from: Some(before.status),
                to: Some(test.status),
                lines: diff_lines(&before.text, &test.text),
                binary: test.binary,
                path: test.path,
            }),
            None => changes.push(Change {
                kind: ChangeKind::Added,
                from: None,
                to: Some(test.status),
                lines: diff_lines("", &test.text),
                binary: test.binary,
                path: test.path,
            }),
        }
    }
    changes.extend(older.into_values().map(|test| Change {
        kind: ChangeKind::Deleted,
        from: Some(test.status),
        to: None,
        lines: diff_lines(&test.text, ""),
        binary: test.binary,
        path: test.path,
    }));
    // A test comes right after the test above it: its path starts with that one's.
    changes.sort_by(|a, b| (&a.binary, &a.path).cmp(&(&b.binary, &b.path)));
    changes
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
        }
    }

    fn save(store: &mut Store, batch: &str, whole_tree: bool, outcomes: &[TestOutcome]) {
        let place = [
            path(&["OrderPlaced"]),
            path(&["OrderPlaced", "OrderPaid"]),
            path(&["OrderPlaced", "OrderCancelled"]),
        ]
        .into_iter()
        .enumerate()
        .map(|(nr, path)| (path, nr))
        .collect();
        store
            .save_tree(batch, BINARY, ROOT, whole_tree, outcomes, &place)
            .unwrap();
    }

    /// What the batch knows: the last name and the text of every test, in order.
    fn known(store: &Store, batch: &str) -> Vec<(String, String)> {
        let batch = store.batch(batch).unwrap().unwrap();
        batch
            .tests
            .into_iter()
            .map(|test| (test.path.last().unwrap().clone(), test.text))
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

    #[test]
    fn part_of_a_tree_that_ran_leaves_the_rest_of_it_as_it_was() {
        let mut store = Store::open(None).unwrap();
        save(&mut store, "work", true, &whole_tree());

        let again = [
            outcome(&["OrderPlaced", "OrderPaid"], Status::Passed, "paid"),
            outcome(&["OrderPlaced"], Status::Passed, "placed again"),
        ];
        save(&mut store, "work", false, &again);

        assert_eq!(
            known(&store, "work"),
            vec![
                (
                    "orders::OrderPlaced".to_string(),
                    "placed again".to_string()
                ),
                ("orders::OrderPaid".to_string(), "paid".to_string()),
                (
                    "orders::OrderCancelled".to_string(),
                    "cancelled".to_string()
                ),
            ]
        );
        let statuses = store.statuses("work", BINARY, ROOT).unwrap();
        assert_eq!(
            statuses[&path(&["OrderPlaced", "OrderPaid"])],
            Status::Passed
        );
    }

    #[test]
    fn whole_tree_that_ran_is_all_the_batch_knows_of_it() {
        let mut store = Store::open(None).unwrap();
        save(&mut store, "work", true, &whole_tree());
        save(
            &mut store,
            "work",
            true,
            &[outcome(&["OrderPlaced"], Status::Passed, "placed")],
        );
        assert_eq!(
            known(&store, "work"),
            vec![("orders::OrderPlaced".to_string(), "placed".to_string())]
        );
    }

    #[test]
    fn test_that_is_no_longer_in_the_code_is_forgotten_when_its_binary_reports() {
        let mut store = Store::open(None).unwrap();
        save(&mut store, "work", true, &whole_tree());
        let existing = HashSet::from([path(&["OrderPlaced"]), path(&["OrderPlaced", "OrderPaid"])]);
        store.binary_reports("work", BINARY, &existing).unwrap();
        store
            .binary_reports("work", "other_binary", &HashSet::new())
            .unwrap();
        assert_eq!(known(&store, "work").len(), 2);
    }

    #[test]
    fn named_batches_are_stored_and_the_default_batch_is_not() {
        let dir = std::env::temp_dir().join(format!("testscribe-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("batches.sqlite");
        {
            let mut store = Store::open(Some(&file)).unwrap();
            save(&mut store, "work", true, &whole_tree());
            save(&mut store, DEFAULT_BATCH, true, &whole_tree());
            let names: Vec<_> = store
                .batches()
                .unwrap()
                .into_iter()
                .map(|b| (b.name, b.stored))
                .collect();
            assert_eq!(
                names,
                vec![("default".to_string(), false), ("work".to_string(), true)]
            );
            store.set_run("work", RunMode::NewAndFailed, None).unwrap();
        }
        let store = Store::open(Some(&file)).unwrap();
        let batches = store.batches().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].name, "work");
        assert_eq!((batches[0].passed, batches[0].failed), (2, 1));
        assert_eq!(batches[0].mode, RunMode::NewAndFailed);
        assert!(store.batch(DEFAULT_BATCH).unwrap().is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn diff_tells_the_tests_that_were_added_deleted_and_modified() {
        let mut store = Store::open(None).unwrap();
        save(&mut store, "before", true, &whole_tree());
        save(
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

        let changes = store.diff("before", "after").unwrap().unwrap();
        let told: Vec<_> = changes
            .iter()
            .map(|change| (change.path.last().unwrap().as_str(), change.kind))
            .collect();
        assert_eq!(
            told,
            vec![
                ("orders::OrderCancelled", ChangeKind::Deleted),
                ("orders::OrderPaid", ChangeKind::Modified),
                ("orders::OrderShipped", ChangeKind::Added),
            ]
        );
        let modified = &changes[1];
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

        assert!(store.diff("before", "nowhere").unwrap().is_none());
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
