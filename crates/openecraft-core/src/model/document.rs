//! Data of `crate::document` that the project file holds (see `docs/architecture.md`).

use openecraft_parts::Shared;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::RefCell;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Document {
    /// File name, e.g. `datasheet.pdf`.
    pub name: String,
    /// File content, base64 encoded (shared: project snapshots compare and clone it
    /// by pointer).
    #[serde(deserialize_with = "read")]
    pub data: Shared<String>,
}

/// Takes the content of a document that is written and returns what is stored in
/// its place (none: the content itself).
pub(crate) type Writer = Box<dyn FnMut(&Document) -> Option<String>>;
/// Gives the content for what was stored in its place (none: the stored text).
pub(crate) type Reader = Box<dyn FnMut(&str) -> Option<Shared<String>>>;

thread_local! {
    static WRITER: RefCell<Option<Writer>> = const { RefCell::new(None) };
    static READER: RefCell<Option<Reader>> = const { RefCell::new(None) };
}

/// Puts the hook back when the work is done, also after a panic.
struct Restore<T: 'static>(&'static std::thread::LocalKey<RefCell<Option<T>>>, Option<T>);

impl<T> Drop for Restore<T> {
    fn drop(&mut self) {
        let before = self.1.take();
        self.0.with(|hook| *hook.borrow_mut() = before);
    }
}

/// Runs `work` with the content of every document it serializes (on this thread)
/// passed to `writer`: the split project format stores embedded files apart from
/// the JSON, wherever the project holds them.
pub(crate) fn with_writer<R>(writer: Writer, work: impl FnOnce() -> R) -> R {
    let _restore = Restore(&WRITER, WRITER.with(|hook| hook.borrow_mut().replace(writer)));
    work()
}

/// Runs `work` with the content of every document it deserializes (on this thread)
/// taken from `reader`.
pub(crate) fn with_reader<R>(reader: Reader, work: impl FnOnce() -> R) -> R {
    let _restore = Restore(&READER, READER.with(|hook| hook.borrow_mut().replace(reader)));
    work()
}

impl Serialize for Document {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let stored = WRITER.with(|hook| hook.borrow_mut().as_mut().and_then(|writer| writer(self)));
        let mut doc = s.serialize_struct("Document", 2)?;
        doc.serialize_field("name", &self.name)?;
        match stored {
            Some(stored) => doc.serialize_field("data", &stored)?,
            None => doc.serialize_field("data", &self.data)?,
        }
        doc.end()
    }
}

fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Shared<String>, D::Error> {
    let stored = String::deserialize(d)?;
    let content = READER.with(|hook| hook.borrow_mut().as_mut().and_then(|reader| reader(&stored)));
    Ok(content.unwrap_or_else(|| stored.into()))
}
