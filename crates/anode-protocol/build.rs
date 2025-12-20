fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &[
                "proto/common.proto",
                "proto/raft.proto",
                "proto/cluster.proto",
                "proto/replication.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
