use remarkable_events::ToolEventSource;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + 'static>>;

fn main() -> Result<()> {
    println!("BLALALAL");
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("/dev/input/event1"));

    let rt = tokio::runtime::Builder::new_current_thread().build()?;

    rt.block_on(async {
        if let Err(err) = run_loop(&target).await {
            eprintln!("Stopping with error: {}", err);
        }
    });

    Ok(())
}

async fn run_loop(target: &str) -> Result<()> {
    let mut tool_events = ToolEventSource::open(target).await?;

    println!("Starting loop");

    loop {
        match tool_events.next().await {
            Ok(tool_ev) => println!("{:?}", tool_ev),
            Err(err) => eprintln!("Error: {}", err),
        }
    }
}
