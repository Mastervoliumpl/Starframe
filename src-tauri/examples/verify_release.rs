use minisign_verify::{PublicKey, Signature};
use std::{env, error::Error, fs, io::Read};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Expected decoded public key, decoded signature and artifact paths".into());
    }
    let key = PublicKey::from_file(&args[0])?;
    let signature = Signature::from_file(&args[1])?;
    let mut verifier = key.verify_stream(&signature)?;
    let mut file = fs::File::open(&args[2])?;
    let mut buffer = [0_u8; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        verifier.update(&buffer[..read]);
    }
    verifier.finalize()?;
    println!("Artifact signature verified.");
    Ok(())
}
