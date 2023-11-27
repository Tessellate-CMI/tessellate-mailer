use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{http::StatusCode, routing::post, Json, Router, Server};
use clap::Parser;
use comrak::Options;
use lettre::{
  message::{header::ContentType, Mailbox},
  Address, AsyncSendmailTransport, AsyncTransport, Message,
};
use serde::{Deserialize, Deserializer};

const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_PORT: u16 = 9876;

#[derive(Parser)]
struct Cli {
  #[arg(short, long, env = "MAILER_HOST", default_value_t = DEFAULT_HOST)]
  host: IpAddr,

  #[arg(short, long, env = "MAILER_PORT", default_value_t = DEFAULT_PORT)]
  port: u16,
}

#[tokio::main]
async fn main() {
  let cli = Cli::parse();

  let app = Router::new()
    .route(
      "/",
      post(|Json(payload): Json<Mail>| async { payload.send().await }),
    )
    .route(
      "/listmonk",
      post(|Json(payload): Json<ListMonkMail>| async {
        Mail::from(payload).send().await
      }),
    );

  println!("Server Running: {}:{}", cli.host, cli.port);

  Server::bind(&SocketAddr::new(cli.host, cli.port))
    .serve(app.into_make_service())
    .await
    .unwrap();
}

#[derive(Deserialize)]
struct Mail {
  subject: String,
  body: String,
  content_type: String,
  from: Mailbox,
  to: Mailbox,
}

impl Mail {
  async fn send(self) -> StatusCode {
    let email = match Message::builder()
      .from(self.from)
      .to(self.to)
      .subject(self.subject)
      .header(if self.content_type == "plain" {
        ContentType::TEXT_PLAIN
      } else {
        ContentType::TEXT_HTML
      })
      .body(if self.content_type == "markdown" {
        comrak::markdown_to_html(&self.body, &Options::default())
      } else {
        self.body
      }) {
      Ok(msg) => msg,
      Err(err) => {
        eprintln!("Mail Error :: {err}");
        return StatusCode::INTERNAL_SERVER_ERROR;
      }
    };

    if let Err(err) = AsyncSendmailTransport::new().send(email).await {
      eprintln!("Mail Error :: {err}");
      StatusCode::INTERNAL_SERVER_ERROR
    } else {
      StatusCode::OK
    }
  }
}

#[derive(Deserialize)]
struct ListMonkMail {
  subject: String,
  body: String,

  #[serde(deserialize_with = "deserialize_content_type")]
  content_type: String,

  #[serde(rename = "campaign", deserialize_with = "deserialize_from")]
  from: Mailbox,

  #[serde(rename = "recipients", deserialize_with = "deserialize_to")]
  to: Mailbox,
}

fn deserialize_content_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
  D: Deserializer<'de>,
{
  let content_type = String::deserialize(deserializer)?;
  Ok(if content_type == "markdown" {
    "html".to_string()
  } else {
    content_type
  })
}

#[derive(Deserialize)]
struct Campaign {
  from_email: Mailbox,
}

fn deserialize_from<'de, D>(deserializer: D) -> Result<Mailbox, D::Error>
where
  D: Deserializer<'de>,
{
  Ok(Campaign::deserialize(deserializer)?.from_email)
}

#[derive(Deserialize)]
struct Recipient {
  email: Address,
  name: String,
}

fn deserialize_to<'de, D>(deserializer: D) -> Result<Mailbox, D::Error>
where
  D: Deserializer<'de>,
{
  let rec = &Vec::<Recipient>::deserialize(deserializer)?[0];
  Ok(Mailbox::new(Some(rec.name.clone()), rec.email.clone()))
}

impl From<ListMonkMail> for Mail {
  fn from(value: ListMonkMail) -> Self {
    Self {
      subject: value.subject,
      body: value.body,
      content_type: value.content_type,
      from: value.from,
      to: value.to,
    }
  }
}
