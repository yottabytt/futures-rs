//! An unbounded set of streams

use core::fmt::{self, Debug};
use core::iter::FromIterator;
use core::pin::Pin;

use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};

use pin_project_lite::pin_project;

use super::assert_stream;
use crate::stream::futures_unordered::{IntoIter, Iter, IterMut, IterPinMut, IterPinRef};
use crate::stream::{FuturesUnordered, StreamExt, StreamFuture};

pin_project! {
    /// An unbounded set of streams
    ///
    /// This "combinator" provides the ability to maintain a set of streams
    /// and drive them all to completion.
    ///
    /// Streams are pushed into this set and their realized values are
    /// yielded as they become ready. Streams will only be polled when they
    /// generate notifications. This allows to coordinate a large number of streams.
    ///
    /// Note that you can create a ready-made `SelectAll` via the
    /// `select_all` function in the `stream` module, or you can start with an
    /// empty set with the `SelectAll::new` constructor.
    #[must_use = "streams do nothing unless polled"]
    pub struct SelectAll<St> {
        #[pin]
        inner: FuturesUnordered<StreamFuture<St>>,
    }
}

impl<St: Debug> Debug for SelectAll<St> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SelectAll {{ ... }}")
    }
}

impl<St: Stream + Unpin> SelectAll<St> {
    /// Constructs a new, empty `SelectAll`
    ///
    /// The returned `SelectAll` does not contain any streams and, in this
    /// state, `SelectAll::poll` will return `Poll::Ready(None)`.
    pub fn new() -> Self {
        Self { inner: FuturesUnordered::new() }
    }

    /// Returns the number of streams contained in the set.
    ///
    /// This represents the total number of in-flight streams.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the set contains no streams
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Push a stream into the set.
    ///
    /// This function submits the given stream to the set for managing. This
    /// function will not call `poll` on the submitted stream. The caller must
    /// ensure that `SelectAll::poll` is called in order to receive task
    /// notifications.
    pub fn push(&mut self, stream: St) {
        self.inner.push(stream.into_future());
    }

    /// Returns an iterator that allows inspecting each future in the set.
    pub fn iter(&self) -> Iter<'_, StreamFuture<St>> {
        self.inner.iter()
    }

    /// Returns an iterator that allows inspecting each future in the set.
    pub fn iter_pin_ref(self: Pin<&'_ Self>) -> IterPinRef<'_, StreamFuture<St>> {
        self.project_ref().inner.iter_pin_ref()
    }

    /// Returns an iterator that allows modifying each future in the set.
    pub fn iter_mut(&mut self) -> IterMut<'_, StreamFuture<St>> {
        self.inner.iter_mut()
    }

    /// Returns an iterator that allows modifying each future in the set.
    pub fn iter_pin_mut(self: Pin<&mut Self>) -> IterPinMut<'_, StreamFuture<St>> {
        self.project().inner.iter_pin_mut()
    }
}

impl<St: Stream + Unpin> Default for SelectAll<St> {
    fn default() -> Self {
        Self::new()
    }
}

impl<St: Stream + Unpin> Stream for SelectAll<St> {
    type Item = St::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match ready!(self.inner.poll_next_unpin(cx)) {
                Some((Some(item), remaining)) => {
                    self.push(remaining);
                    return Poll::Ready(Some(item));
                }
                Some((None, _)) => {
                    // `FuturesUnordered` thinks it isn't terminated
                    // because it yielded a Some.
                    // We do not return, but poll `FuturesUnordered`
                    // in the next loop iteration.
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

impl<St: Stream + Unpin> FusedStream for SelectAll<St> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

/// Convert a list of streams into a `Stream` of results from the streams.
///
/// This essentially takes a list of streams (e.g. a vector, an iterator, etc.)
/// and bundles them together into a single stream.
/// The stream will yield items as they become available on the underlying
/// streams internally, in the order they become available.
///
/// Note that the returned set can also be used to dynamically push more
/// futures into the set as they become available.
///
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
pub fn select_all<I>(streams: I) -> SelectAll<I::Item>
where
    I: IntoIterator,
    I::Item: Stream + Unpin,
{
    let mut set = SelectAll::new();

    for stream in streams {
        set.push(stream);
    }

    assert_stream::<<I::Item as Stream>::Item, _>(set)
}

impl<St: Stream + Unpin> FromIterator<St> for SelectAll<St> {
    fn from_iter<T: IntoIterator<Item = St>>(iter: T) -> Self {
        select_all(iter)
    }
}

impl<St: Stream + Unpin> Extend<St> for SelectAll<St> {
    fn extend<T: IntoIterator<Item = St>>(&mut self, iter: T) {
        for st in iter {
            self.push(st)
        }
    }
}

impl<St: Stream + Unpin> IntoIterator for SelectAll<St> {
    type Item = StreamFuture<St>;
    type IntoIter = IntoIter<StreamFuture<St>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a, St: Stream + Unpin> IntoIterator for &'a SelectAll<St> {
    type Item = &'a StreamFuture<St>;
    type IntoIter = Iter<'a, StreamFuture<St>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, St: Stream + Unpin> IntoIterator for &'a mut SelectAll<St> {
    type Item = &'a mut StreamFuture<St>;
    type IntoIter = IterMut<'a, StreamFuture<St>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
