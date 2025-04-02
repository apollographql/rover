use camino::Utf8PathBuf;

pub fn display_welcome_message() {
    println!("\nWelcome! This command helps you initialize a federated GraphQL API in your current directory.");
    println!("\nTo learn more about init and each use case, run `rover init -h` or visit https://www.apollographql.com/docs/rover/commands/init");
}

pub fn display_project_created_message(project_name: &str, artifacts: &Vec<Utf8PathBuf>, graph_id: &str, api_key: &str) {
  println!("\n=> All set! Your project `{}` has been created. Please review details below to see what was generated.", project_name);
  
  // Display created files
  println!("\nProject directory");
  for artifact in artifacts {
      println!("✓ {}", artifact);
  }
  
  // Display credentials
  println!("\nGraphOS credentials for your GraphQL API");
  println!("✓ APOLLO_GRAPH_REF={}@current (Formatted graph-id@variant, references a GraphQL API in the Apollo GraphOS platform)", graph_id);
  println!("✓ APOLLO_KEY={} (This is your project's API key, also known as a graph API key)", api_key);
  
  // Display next steps
  println!("\n️▲ Before you proceed:");
  println!("- Set your graph API key as an environment variable (learn more about env vars by running `rover docs open config`)");
  println!("- Save your graph ref (You can also get it from Studio by visiting your graph variant's home page)");
  
  println!("\nNext steps Run the following command to start a local development session:  $ rover dev --supergraph-config supergraph.yaml  For more information, check out `getting-started.md`.");
}