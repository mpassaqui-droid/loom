//! Dev tool: open LOOM's MIDI ports and list them.  cargo run -p loom-audio --example midi_ports
fn main() {
    match loom_audio::midi::MidiIo::start(|m| println!("reçu : {m:?}")) {
        Ok(io) => {
            println!("{}", io.description);
            std::thread::sleep(std::time::Duration::from_millis(800));
            let _ = std::process::Command::new("aconnect").arg("-l").status();
        }
        Err(e) => println!("MIDI indisponible : {e}"),
    }
}
