fn main() -> Result<(), Box<dyn std::error::Error>> {
    // tonic_build::configure()
    //     .out_dir("./src/models")
    //     .compile(&["proto/gateway.proto"], &["proto"])?;
    tonic_build::compile_protos("proto/gateway.proto")?;
    Ok(())
}
