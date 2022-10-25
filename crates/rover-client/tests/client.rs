#[cfg(test)]
mod tests {
    #[test]
    fn schema_at_stable_path() {
        // this path must remain stable as we upload it as part of our release process
        assert!(std::fs::metadata("./.schema/schema.graphql").is_ok())
    }
}
