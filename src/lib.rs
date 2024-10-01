use pin_project::pin_project;
use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

/// A `Future` combinator that applies a timeout with a custom callback when the timeout elapses
#[pin_project]
pub struct InspectTimeout<Fut, F, T> {
    #[pin]
    fut: Fut,
    #[pin]
    delay: tokio::time::Sleep,
    elapse_fn: Option<F>,
    delay_state: DelayState,
    _phantom: PhantomData<T>,
}

impl<Fut, F, T> InspectTimeout<Fut, F, T>
where
    F: FnOnce(),
{
    pub fn new(fut: Fut, dur: Duration, elapse_fn: F) -> Self {
        Self {
            fut,
            delay: tokio::time::sleep(dur),
            elapse_fn: Some(elapse_fn),
            delay_state: DelayState::Idle,
            _phantom: PhantomData,
        }
    }

    fn call_elapse_fn(self: Pin<&mut Self>) {
        let this = self.project();

        this.elapse_fn
            .take()
            .expect("elapse_fn must be called once")();

        *this.delay_state = DelayState::Completed;
    }
}

impl<Fut, F, T> Future for InspectTimeout<Fut, F, T>
where
    Fut: Future<Output = T>,
    F: FnOnce(),
{
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();

        if let Poll::Ready(r) = this.fut.poll(cx) {
            return Poll::Ready(r);
        };

        // Check if the timer has been started before; if not, start it.
        match this.delay_state {
            DelayState::Idle => match this.delay.poll(cx) {
                Poll::Ready(_) => {
                    self.as_mut().call_elapse_fn();
                }
                Poll::Pending => *this.delay_state = DelayState::Running,
            },
            DelayState::Running => {
                if this.delay.poll(cx).is_ready() {
                    self.as_mut().call_elapse_fn();
                }
            }
            DelayState::Completed => {}
        };

        Poll::Pending
    }
}

pub trait InspectTimeoutExt<Fut, F, T>
where
    Fut: Future<Output = T>,
    F: FnOnce(),
{
    /// Set a callback in case the `Future` does not complete within a specified period of time
    fn inspect_timeout(self, dur: Duration, elapse_fn: F) -> InspectTimeout<Fut, F, T>;
}

impl<Fut, F, T> InspectTimeoutExt<Fut, F, T> for Fut
where
    Fut: Future<Output = T>,
    F: FnOnce(),
{
    fn inspect_timeout(self, dur: Duration, elapse_fn: F) -> InspectTimeout<Fut, F, T> {
        InspectTimeout::new(self, dur, elapse_fn)
    }
}

enum DelayState {
    Idle,
    Running,
    Completed,
}
