use std::task::Poll;

pin_project_lite::pin_project! {
    pub struct BodyWithSize<B> {
        #[pin]
        body: B,
        size_hint: Option<http_body::SizeHint>,
    }
}

impl<B> BodyWithSize<B> {
    pub fn new(body: B) -> Self {
        Self {
            body,
            size_hint: None,
        }
    }

    pub fn with_size_hint(mut self, size_hint: Option<http_body::SizeHint>) -> Self {
        self.size_hint = size_hint;
        self
    }
}

impl<B> http_body::Body for BodyWithSize<B>
where
    B: http_body::Body,
{
    type Data = B::Data;

    type Error = B::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        this.body.poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.size_hint
            .clone()
            .unwrap_or_else(|| self.body.size_hint())
    }
}

pin_project_lite::pin_project! {
    pub struct StreamBody<S> {
        #[pin]
        stream: S,
    }
}

impl<S> StreamBody<S> {
    pub fn new(stream: S) -> Self {
        Self { stream }
    }
}

impl<S> http_body::Body for StreamBody<S>
where
    S: futures::TryStream,
    S::Ok: bytes::Buf,
{
    type Data = S::Ok;

    type Error = S::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        this.stream.try_poll_next(cx).map_ok(http_body::Frame::data)
    }

    fn size_hint(&self) -> http_body::SizeHint {
        let (lower, upper) = self.stream.size_hint();

        let mut size_hint = http_body::SizeHint::new();
        if let Ok(lower) = lower.try_into() {
            size_hint.set_lower(lower);
        }
        if let Some(upper) = upper
            && let Ok(upper) = upper.try_into()
        {
            size_hint.set_upper(upper);
        }

        size_hint
    }
}
