# re_crypto

Native TLS provider selection shared by Rerun's HTTP, gRPC, and authentication code.
Ring is the default; disable default features and enable `crypto-aws-lc-rs` to use AWS-LC.
AWS-LC takes precedence when both features are enabled, and an application-installed TLS provider is always preserved.
Neither native backend is built for WebAssembly.
