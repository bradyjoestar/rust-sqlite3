//! `rust-sqlite3` is a rustic binding to the [sqlite3 API][].
//!
//! [sqlite3 API]: http://www.sqlite.org/c3ref/intro.html
//!
//! ```rust
//! extern crate sqlite3;
//! extern crate time;
//!
//! use time::Timespec;
//!
//! use sqlite3::{DatabaseConnection, Row, SqliteResult};
//!
//! #[deriving(Show)]
//! struct Person {
//!     id: i32,
//!     name: String,
//!     time_created: Timespec,
//!     // TODO: data: Option<Vec<u8>>
//! }
//!
//! fn io() -> SqliteResult<()> {
//!     let mut conn = try!(DatabaseConnection::new());
//!
//!     try!(conn.exec("CREATE TABLE person (
//!                  id              SERIAL PRIMARY KEY,
//!                  name            VARCHAR NOT NULL,
//!                  time_created    TIMESTAMP NOT NULL
//!                )"));
//!
//!     let me = Person {
//!         id: 0,
//!         name: "Dan".to_string(),
//!         time_created: time::get_time(),
//!     };
//!     try!(try!(conn.prepare("INSERT INTO person (name, time_created)
//!                   VALUES ($1, $2)")).update(
//!               [&me.name, &me.time_created]));
//!
//!     let mut stmt = try!(conn.prepare("SELECT id, name, time_created FROM person"));
//!     let mut rows = try!(stmt.query([]));
//!     loop {
//!         match rows.step() {
//!             Row(ref mut row) => {
//!                 let person = Person {
//!                     id: row.get(0u),
//!                     name: row.get(1u),
//!                     time_created: row.get(2u)
//!                 };
//!                 println!("Found person {}", person);
//!             },
//!             _ => break
//!         }
//!     }
//!     Ok(())
//! }
//!
//! fn main() {
//!   io().unwrap();
//! }
//! ```

#![crate_name = "sqlite3"]
#![crate_type = "lib"]
#![feature(unsafe_destructor)]

extern crate libc;
extern crate time;

use std::fmt::Show;

pub use core::{DatabaseConnection, PreparedStatement, ResultSet, ResultRow};
pub use types::{FromSql, ToSql};

pub mod core;
pub mod types;

/// bindgen-bindings to libsqlite3
#[allow(non_camel_case_types, dead_code)]
pub mod ffi;

pub mod access;


impl<'db> core::PreparedStatement<'db> {
    /// Execute a query after binding any parameters.
    ///
    /// No [number of rows modified][changes] is reported when the
    /// statement is done. (See `ResultSet::step()`.)
    ///
    /// [changes]: http://www.sqlite.org/c3ref/changes.html
    pub fn query(&'db mut self, values: &[&ToSql])
                 -> SqliteResult<ResultSet<'db>> {
        self.execute(false, convert(values))
    }

    /// Execute a statement after binding any parameters.
    ///
    /// When the statement is done, The [number of rows
    /// modified][changes] is reported. (See `ResultSet::step()`.)
    ///
    /// [changes]: http://www.sqlite.org/c3ref/changes.html
    pub fn update(&'db mut self, values: &[&ToSql])
                  -> SqliteResult<ResultSet<'db>> {
        self.execute(true, convert(values))
    }
}

fn convert<'a>(values: &'a[&ToSql]) -> Vec<ParameterValue> {
    values.iter().map(|v| v.to_sql()).collect()
}


impl<'s, 'r> core::ResultRow<'s, 'r> {
    pub fn get<I: RowIndex + Show + Clone, T: FromSql>(&mut self, idx: I) -> T {
        match self.get_opt(idx.clone()) {
            Ok(ok) => ok,
            Err(err) => fail!("retrieving column {}: {}", idx, err)
        }
    }

    pub fn get_opt<I: RowIndex, T: FromSql>(&mut self, idx: I) -> SqliteResult<T> {
        match idx.idx(self) {
            Some(idx) => FromSql::from_sql(self, idx),
            None => Err(SQLITE_MISUSE)
        }
    }

}

/// A trait implemented by types that can index into columns of a row.
///
/// *inspired by sfackler's [RowIndex][]*
/// [RowIndex]: http://www.rust-ci.org/sfackler/rust-postgres/doc/postgres/trait.RowIndex.html
pub trait RowIndex {
    fn idx(&self, row: &mut ResultRow) -> Option<uint>;
}

impl RowIndex for uint {
    fn idx(&self, _row: &mut ResultRow) -> Option<uint> { Some(*self) }
}

impl RowIndex for &'static str {
    fn idx(&self, row: &mut ResultRow) -> Option<uint> {
        let mut ixs = range(0, row.column_count());
        ixs.find(|ix| row.with_column_name(*ix, false, |name| name == *self))
    }
}


// ref http://www.sqlite.org/c3ref/c_abort.html
#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
enum SqliteOk {
    SQLITE_OK = 0
}


/// The type used for returning and propagating sqlite3 errors.
#[must_use]
pub type SqliteResult<T> = Result<T, SqliteError>;

/// Result codes for errors.
///
/// cf. [sqlite3 result codes][codes].
///
/// Note `SQLITE_OK` is not included; we use `Ok(...)` instead.
///
/// Likewise, in place of `SQLITE_ROW` and `SQLITE_DONE`, we return
/// `Some(...)` or `None` from `ResultSet::next()`.
///
/// [codes]: http://www.sqlite.org/c3ref/c_abort.html
#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
pub enum SqliteError {
    SQLITE_ERROR     =  1,
    SQLITE_INTERNAL  =  2,
    SQLITE_PERM      =  3,
    SQLITE_ABORT     =  4,
    SQLITE_BUSY      =  5,
    SQLITE_LOCKED    =  6,
    SQLITE_NOMEM     =  7,
    SQLITE_READONLY  =  8,
    SQLITE_INTERRUPT =  9,
    SQLITE_IOERR     = 10,
    SQLITE_CORRUPT   = 11,
    SQLITE_NOTFOUND  = 12,
    SQLITE_FULL      = 13,
    SQLITE_CANTOPEN  = 14,
    SQLITE_PROTOCOL  = 15,
    SQLITE_EMPTY     = 16,
    SQLITE_SCHEMA    = 17,
    SQLITE_TOOBIG    = 18,
    SQLITE_CONSTRAINT= 19,
    SQLITE_MISMATCH  = 20,
    SQLITE_MISUSE    = 21,
    SQLITE_NOLFS     = 22,
    SQLITE_AUTH      = 23,
    SQLITE_FORMAT    = 24,
    SQLITE_RANGE     = 25,
    SQLITE_NOTADB    = 26
}

#[deriving(Show, PartialEq, Eq, FromPrimitive)]
#[allow(non_camel_case_types)]
// TODO: use, test this
enum SqliteLogLevel {
    SQLITE_NOTICE    = 27,
    SQLITE_WARNING   = 28,
}

/// Outcome of evaluating one step of a statement.
pub enum StepOutcome<'s, 'r> {
    /// Step yielded a row.
    Row(ResultRow<'s, 'r>),
    /// Statement is done; changes are reported if requested.
    Done(Option<uint>),
    /// Error outcome.
    Error(SqliteError)
}


/// A value for binding to a parameter in a `PreparedStatement`
///
/// cf. [Parameters][]
/// [Parameters]: http://www.sqlite.org/lang_expr.html#varparam
///
///  - *TODO: value?*
///  - *TODO: zeroblob?*
#[unstable]
#[deriving(Show, PartialEq)]
pub enum ParameterValue {
    Blob(Vec<u8>),
    Float64(f64),
    Integer(int),
    Integer64(i64),
    Null,
    Text(String),
}

#[cfg(test)]
mod tests {
    use super::{DatabaseConnection, SqliteResult, ResultSet};
    use super::Row;

    #[test]
    fn db_new_types() {
        DatabaseConnection::new().unwrap();
    }

    #[test]
    fn stmt_new_types() {
        fn go() -> SqliteResult<()> {
            let mut db = try!(DatabaseConnection::new());
            db.prepare("select 1 + 1").map( |_s| () )
        }
        go().unwrap();
    }


    fn with_query<T>(sql: &str, f: |rows: &mut ResultSet| -> T) -> SqliteResult<T> {
        let mut db = try!(DatabaseConnection::new());
        let mut s = try!(db.prepare(sql));
        let mut rows = try!(s.query([]));
        Ok(f(&mut rows))
    }

    #[test]
    fn query_two_rows() {
        fn go() -> SqliteResult<(uint, i32)> {
            let mut count = 0;
            let mut sum = 0;

            with_query("select 1
                       union all
                       select 2", |rows| {
                loop {
                    match rows.step() {
                        Row(ref mut row) => {
                            count += 1;
                            sum += row.get(0u)
                        },
                        _ => break
                    }
                }
                (count, sum)
            })
        }
        assert_eq!(go(), Ok((2, 3)))
    }

    #[test]
    fn named_rowindex() {
        fn go() -> SqliteResult<(uint, i32)> {
            let mut count = 0;
            let mut sum = 0;

            with_query("select 1 as col1
                       union all
                       select 2", |rows| {
                loop {
                    match rows.step() {
                        Row(ref mut row) => {
                            count += 1;
                            sum += row.get("col1")
                        },
                        _ => break
                    }
                }
                (count, sum)
            })
        }
        assert_eq!(go(), Ok((2, 3)))
    }
}


#[cfg(test)]
mod bind_tests {
    use super::DatabaseConnection;
    use super::{SqliteResult, Integer, Text};
    use super::{Row, Done};

    #[test]
    fn bind_fun() {
        fn go() -> SqliteResult<()> {
            let mut database = try!(DatabaseConnection::new());

            try!(database.exec(
                "BEGIN;
                CREATE TABLE test (id int, name text, address text);
                INSERT INTO test (id, name, address) VALUES (1, 'John Doe', '123 w Pine');
                COMMIT;"));

            {
                let mut tx = try!(database.prepare(
                    "INSERT INTO test (id, name, address) VALUES (?, ?, ?)"));
                let mut rows = try!(tx.execute(true,
                                               vec!(Integer(2),
                                                    Text("Jane Doe".to_string()),
                                                    Text("345 e Walnut".to_string()))));
                assert_eq!(match rows.step() { Done(changed) => changed, _ => None },
                           Some(1));
            }

            let mut q = try!(database.prepare("select * from test order by id"));
            let mut rows = try!(q.query([]));
            match rows.step() {
                Row(ref mut row) => {
                    assert_eq!(row.get::<uint, int>(0), 1);
                    // TODO let name = q.get_text(1);
                    // assert_eq!(name.as_slice(), "John Doe");
                },
                _ => fail!()
            }

            match rows.step() {
                Row(ref mut row) => {
                    assert_eq!(row.get::<uint, int>(0), 2);
                    //TODO let addr = q.get_text(2);
                    // assert_eq!(addr.as_slice(), "345 e Walnut");
                },
                _ => fail!()
            }
            Ok(())
        }
        match go() {
            Ok(_) => (),
            Err(e) => fail!("oops! {}", e)
        }
    }
}
