// workbench.rs

use std::env;

use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use lettre::{Message, SmtpTransport, Transport};
use tracing::info;

fn main() {
    env_logger::init();

    // log our first message
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    info!("Hello, workbench v{VERSION}!");

    // read e-mail config from the environment
    let body = env::var("MAIL_BODY").expect("Expected environment variable: MAIL_BODY");
    let password = env::var("MAIL_PASSWORD").expect("Expected environment variable: MAIL_PASSWORD");
    let port_str = env::var("MAIL_PORT").expect("Expected environment variable: MAIL_PORT");
    let recipient = env::var("MAIL_TO").expect("Expected environment variable: MAIL_TO");
    let server = env::var("MAIL_SERVER").expect("Expected environment variable: MAIL_SERVER");
    let subject = env::var("MAIL_SUBJECT").expect("Expected environment variable: MAIL_SUBJECT");
    let username = env::var("MAIL_USERNAME").expect("Expected environment variable: MAIL_USERNAME");

    let port: u16 = port_str
        .parse()
        .expect("Unable to parse MAIL_PORT as a u16");

    // prepare an e-mail message to be sent
    let email = Message::builder()
        .from(username.parse().unwrap())
        .to(recipient.parse().unwrap())
        .subject(subject)
        .body(body)
        .unwrap();

    let _creds = Credentials::new(username, password);

    let mailer = SmtpTransport::builder_dangerous(server)
        .port(port)
        .tls(Tls::None) // Tls::Required if STARTTLS
        // .credentials(_creds)
        .build();

    // send the message and report on the results
    match mailer.send(&email) {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => eprintln!("Could not send email: {:?}", e),
    }
}
