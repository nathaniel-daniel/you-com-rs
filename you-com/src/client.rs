use crate::ChatMessage;
use crate::ChatResponseStream;
use crate::Error;
use futures_util::stream::TryStreamExt;
use nd_tokio_sse_codec::SseCodec;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::ACCEPT;
use reqwest::header::ACCEPT_LANGUAGE;
use reqwest::header::CACHE_CONTROL;
use reqwest::header::CONNECTION;
use reqwest::header::HOST;
use reqwest::header::REFERER;
use reqwest::header::USER_AGENT;
use tokio_util::codec::FramedRead;
use tokio_util::io::StreamReader;
use url::Url;

static USER_AGENT_VALUE: HeaderValue = HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36");
static ACCEPT_LANGUAGE_VALUE: HeaderValue = HeaderValue::from_static("en-US,en;q=0.9");

/// A you client
#[derive(Debug, Clone)]
pub struct Client {
    /// The inner http client
    pub client: reqwest::Client,
}

impl Client {
    /// Make a new client
    pub fn new() -> Self {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(USER_AGENT, USER_AGENT_VALUE.clone());
        default_headers.insert(ACCEPT_LANGUAGE, ACCEPT_LANGUAGE_VALUE.clone());

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .http1_title_case_headers()
            .build()
            .expect("failed to build client");

        Self { client }
    }

    /// Chat with you.com.
    pub async fn chat(
        &self,
        question: &str,
        chat: &[ChatMessage],
    ) -> Result<ChatResponseStream, Error> {
        let chat_json = serde_json::to_string(&chat).map_err(|error| Error::Json { error })?;

        let url = "https://you.com/api/streamingSearch";
        let mut url = Url::parse(url)?;
        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("q", question);
            query_pairs.append_pair("page", "1");
            query_pairs.append_pair("count", "10");
            query_pairs.append_pair("safeSearch", "Off"); // "Moderate", "Strict", or "Off"
            query_pairs.append_pair("mkt", "en-US");
            query_pairs.append_pair("domain", "youchat");
            query_pairs.append_pair("use_personalization_extraction", "true");
            query_pairs.append_pair("pastChatLength", itoa::Buffer::new().format(chat.len()));
            query_pairs.append_pair("selectedChatMode", "default");
            query_pairs.append_pair("chat", &chat_json);
        }

        // Order matters for header serialization, apparently.
        let response = self
            .client
            .get(url.as_str())
            .header(HOST, "you.com")
            .header(ACCEPT, "text/event-stream")
            .header(ACCEPT_LANGUAGE, ACCEPT_LANGUAGE_VALUE.clone())
            .header(CACHE_CONTROL, "no-cache")
            .header(
                REFERER,
                "https://you.com/search?fromSearchBar=true&tbm=youchat",
            )
            .header(CONNECTION, "keep-alive")
            .header("Pragma", "no-cache")
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-origin")
            .send()
            .await?
            .error_for_status()?;
        let bytes_stream = response.bytes_stream().map_err(std::io::Error::other);
        let stream_reader = StreamReader::new(bytes_stream);
        let codec = SseCodec::new();
        let reader = FramedRead::new(stream_reader, codec);
        let chat_response_stream = ChatResponseStream::new(Box::pin(reader));

        Ok(chat_response_stream)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
