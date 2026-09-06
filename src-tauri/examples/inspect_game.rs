fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Pass a game installation folder to inspect read-only.")?;
    let game = starframe::game::inspect(&std::path::PathBuf::from(path))?;
    println!("{}\n{}\n{}", game.edition, game.build, game.path);
    #[cfg(windows)]
    println!(
        "Process: {:?}",
        starframe::game::classify(
            &std::path::PathBuf::from(game.executable),
            starframe::windows_game::processes()
        )
    );
    Ok(())
}
