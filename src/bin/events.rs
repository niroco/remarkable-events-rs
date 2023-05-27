use anyhow::Result;
use remarkable_events::EventSource;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    println!("BLALALAL");
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("/dev/input/event1"));

    run_loop(target).await?;

    Ok(())
}

async fn run_loop(target: String) -> Result<()> {
    let mut tool_events = EventSource::open(target).await?;

    println!("Starting loop");

    loop {
        match tool_events.next().await {
            Ok(tool_ev) => println!("{:?}", tool_ev),
            Err(err) => eprintln!("Error: {}", err),
        }
    }
}
