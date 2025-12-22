//! Integration test runner
//!
//! This file serves as the entry point for integration tests.
//! It re-exports all test modules from the library.

// Import all test modules
mod s3_basic {
    pub use anode_integration_tests::s3_basic::*;
}

mod s3_multipart {
    pub use anode_integration_tests::s3_multipart::*;
}

mod cluster_ops {
    pub use anode_integration_tests::cluster_ops::*;
}

mod failover {
    pub use anode_integration_tests::failover::*;
}

mod performance {
    pub use anode_integration_tests::performance::*;
}

mod parquet {
    pub use anode_integration_tests::parquet::*;
}
