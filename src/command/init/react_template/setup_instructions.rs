use std::path::PathBuf;
use console::{style, Emoji};
use crate::command::init::react_template::environment_checker::{SafeEnvironmentChecker, PackageManager};

static SPARKLES: Emoji<'_, '_> = Emoji("✨ ", "");
static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "");
static BOOKS: Emoji<'_, '_> = Emoji("📚 ", "");
static WRENCH: Emoji<'_, '_> = Emoji("🔧 ", "");
static ARROW: Emoji<'_, '_> = Emoji("➡️  ", "");

pub struct SetupInstructions {
    pub project_path: PathBuf,
    pub project_name: String,
    pub has_node: bool,
    pub has_npm: bool,
    pub package_manager: PackageManager,
}

impl SetupInstructions {
    pub fn new(project_path: PathBuf, project_name: String) -> Self {
        let has_node = SafeEnvironmentChecker::check_node_exists();
        let has_npm = SafeEnvironmentChecker::check_npm_exists();
        let package_manager = SafeEnvironmentChecker::detect_package_manager();

        Self {
            project_path,
            project_name,
            has_node,
            has_npm,
            package_manager,
        }
    }

    pub fn display(&self) {
        println!();
        println!("{} {}", SPARKLES, style("Your Apollo React app has been created!").green().bold());
        println!();
        
        self.print_project_info();
        self.print_next_steps();
        self.print_additional_info();
        
        println!();
        println!("{} {}", ROCKET, style("Happy coding!").green().bold());
        println!();
    }

    fn print_project_info(&self) {
        println!("{} {}", WRENCH, style("Project Details:").yellow().bold());
        println!("   {} {}", style("Name:").bold(), self.project_name);
        println!("   {} {}", style("Path:").bold(), self.project_path.display());
        println!("   {} {}", style("Package Manager:").bold(), self.package_manager.name());
        println!();
    }

    fn print_next_steps(&self) {
        println!("{} {}", ARROW, style("Next Steps:").yellow().bold());
        println!();

        let mut step = 1;

        // Check for Node.js/npm
        if !self.has_node || !self.has_npm {
            println!("{}. {}", step, style("Install Node.js (includes npm):").bold());
            println!("   Visit: {}", style("https://nodejs.org/").blue().underlined());
            println!("   Recommended: Use the LTS version");
            println!();
            step += 1;
        }

        // Navigate to directory
        println!("{}. {}", step, style("Navigate to your project:").bold());
        println!("   {}", style(format!("cd {}", self.project_path.display())).cyan());
        println!();
        step += 1;

        // Install dependencies
        println!("{}. {}", step, style("Install dependencies:").bold());
        println!("   {}", style(self.package_manager.install_command()).cyan());
        println!("   This will install React, Apollo Client, and development tools");
        println!();
        step += 1;

        // Start development server
        println!("{}. {}", step, style("Start the development server:").bold());
        println!("   {}", style(self.package_manager.dev_command()).cyan());
        println!("   Your app will open at {}", style("http://localhost:5173").blue().underlined());
        println!();
    }

    fn print_additional_info(&self) {
        println!("{} {}", BOOKS, style("Additional Information:").yellow().bold());
        println!("   • GraphQL endpoint is configured in {}", style(".env").cyan());
        println!("   • Apollo Client setup is in {}", style("src/apollo/client.ts").cyan());
        println!("   • Example query is in {}", style("src/apollo/queries/example.ts").cyan());
        println!("   • Run {} to update your GraphQL schema", style("npm run codegen").cyan());
        println!();
        
        println!("{}", style("Available Scripts:").bold());
        println!("   {} - Start development server", style(self.package_manager.dev_command()).cyan());
        println!("   {} - Build for production", style(self.get_build_command()).cyan());
        println!("   {} - Run linter", style(self.get_lint_command()).cyan());
        println!("   {} - Run type checker", style(self.get_type_check_command()).cyan());
        println!();

        println!("{}", style("Learn More:").bold());
        println!("   • Apollo Client: {}", style("https://www.apollographql.com/docs/react/").blue().underlined());
        println!("   • React: {}", style("https://react.dev/").blue().underlined());
        println!("   • Vite: {}", style("https://vitejs.dev/").blue().underlined());
        println!("   • Rover: {}", style("https://www.apollographql.com/docs/rover/").blue().underlined());
    }

    fn get_build_command(&self) -> String {
        match self.package_manager {
            PackageManager::Npm => "npm run build".to_string(),
            PackageManager::Yarn => "yarn build".to_string(),
            PackageManager::Pnpm => "pnpm build".to_string(),
            PackageManager::Bun => "bun run build".to_string(),
        }
    }

    fn get_lint_command(&self) -> String {
        match self.package_manager {
            PackageManager::Npm => "npm run lint".to_string(),
            PackageManager::Yarn => "yarn lint".to_string(),
            PackageManager::Pnpm => "pnpm lint".to_string(),
            PackageManager::Bun => "bun run lint".to_string(),
        }
    }

    fn get_type_check_command(&self) -> String {
        match self.package_manager {
            PackageManager::Npm => "npm run type-check".to_string(),
            PackageManager::Yarn => "yarn type-check".to_string(),
            PackageManager::Pnpm => "pnpm type-check".to_string(),
            PackageManager::Bun => "bun run type-check".to_string(),
        }
    }

    pub fn display_warnings(&self) {
        if !self.has_node || !self.has_npm {
            println!();
            println!("{} {}", style("⚠️").yellow(), style("Warning:").yellow().bold());
            if !self.has_node {
                println!("   Node.js was not found on your system.");
            }
            if !self.has_npm {
                println!("   npm was not found on your system.");
            }
            println!("   Please install Node.js before running the project.");
            println!("   Visit: {}", style("https://nodejs.org/").blue().underlined());
            println!();
        }
    }

    pub fn display_success_summary(&self) {
        println!();
        println!("{}", style("━".repeat(60)).dim());
        println!();
        println!("{} {}", style("✅").green(), style("Project created successfully!").green().bold());
        println!();
        println!("{} {}", style("📁").blue(), style("Project:").bold());
        println!("   {}", self.project_name);
        println!();
        println!("{} {}", style("📍").blue(), style("Location:").bold());
        println!("   {}", self.project_path.display());
        println!();
        println!("{} {}", style("🚀").blue(), style("Quick Start:").bold());
        println!("   {}", style(format!("cd {}", self.project_path.display())).cyan());
        println!("   {}", style(self.package_manager.install_command()).cyan());
        println!("   {}", style(self.package_manager.dev_command()).cyan());
        println!();
        println!("{}", style("━".repeat(60)).dim());
    }
}