mod client;
mod model;

pub use self::client::Client;
pub use self::model::ChatEvent;
pub use self::model::ChatMessage;
pub use self::model::ChatResponseStream;

/// Library error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Reqwest http error
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    /// A JSON error occured
    #[error("json error")]
    Json {
        #[source]
        error: serde_json::Error,
    },

    /// An sse event was not valid
    #[error("invalid sse event")]
    InvalidSseEvent(#[from] nd_tokio_sse_codec::SseCodecError),

    /// Invalid Json event data was encountered
    #[error("invalid JSON event data")]
    InvalidJsonEventData {
        #[source]
        error: serde_json::Error,
    },

    /// An SSE Event was missing data.
    #[error("sse event missing data")]
    SseEventMissingData,

    /// Invalid url
    #[error("invalid url")]
    InvalidUrl(#[from] url::ParseError),
}

#[cfg(test)]
mod test {
    use super::*;

    // you.com blocks the IP of Github's CI
    #[ignore]
    #[tokio::test]
    async fn it_works() {
        let client = Client::new();

        let mut chat = Vec::new();
        let question_1 = "Hello, How are you?";
        let mut stream_1 = client
            .chat(question_1, &chat)
            .await
            .expect("failed to get ai stream");
        let response_1 = stream_1
            .collect_answer()
            .await
            .expect("failed to get ai answer");
        dbg!(&response_1);
        chat.push(ChatMessage {
            question: question_1.into(),
            answer: response_1,
        });

        let mut stream_2 = client
            .chat("What was my first message?", &chat)
            .await
            .expect("failed to get ai stream");
        let response_2 = stream_2
            .collect_answer()
            .await
            .expect("failed to get ai answer");
        dbg!(&response_2);
        assert!(response_2.contains(question_1));
    }
}
