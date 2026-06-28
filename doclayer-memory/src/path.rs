use bson::Bson;

/// A dot-notation path that can be resolved against a BSON document.
pub(crate) struct BsonPath<'a>(&'a str);

impl<'a> BsonPath<'a> {
    pub(crate) fn new(path: &'a str) -> Self {
        Self(path)
    }

    /// Descends into `doc` following each dot-separated segment, returning the value at the path
    /// or `None` if any segment is missing or an intermediate value is not a document.
    pub(crate) fn resolve<'b>(&self, doc: &'b Bson) -> Option<&'b Bson> {
        self.0.split('.').try_fold(doc, |current, segment| {
            current.as_document()?.get(segment)
        })
    }
}
