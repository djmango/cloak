# Invisibility Inc - Cloak

Cloak is a high-performance Rust-based backend server powering the Invisibility AI assistant application. Built with Actix-Web and Shuttle.rs.

## 🚀 Features

- **High Performance**: Built with Rust for maximum speed and reliability
- **AI Integration**: Seamless integration with multiple AI providers (Keywords AI, OpenAI)
- **Authentication**: Secure user authentication via WorkOS
- **Memory System**: Sophisticated memory management for personalized user experiences
- **Payment Processing**: Stripe integration for subscription management
- **Scheduled Jobs**: Background tasks for memory generation and maintenance
- **API Documentation**: Auto-generated API documentation with Utoipa
- **Database Integration**: PostgreSQL with SQLx for type-safe queries
- **AWS S3 Integration**: File storage and management
- **Caching**: Efficient in-memory caching with Moka

## 📋 Prerequisites

- Rust and Cargo
- Shuttle.rs CLI
- PostgreSQL database
- Various API keys (see Configuration section)

## 🛠️ Setup

### Install Dependencies

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Shuttle
curl -sSfL https://www.shuttle.rs/install | bash

# Install project dependencies
cargo install
```

### Database Setup

1. Set up a PostgreSQL database
2. Create a `Secrets.toml` file in the root of the project (use `Secrets.dev.toml` as a template)
3. Run database migrations:

```bash
sqlx migrate run
```

### Configuration

Create a `Secrets.toml` file with the following environment variables:

```toml
DB_CONNECTION_URI = "postgresql://username:password@localhost:5432/database_name"
KEYWORDS_API_KEY = "your_keywords_api_key"
WORKOS_API_KEY = "your_workos_api_key"
WORKOS_CLIENT_ID = "your_workos_client_id"
JWT_SECRET = "your_jwt_secret"
AWS_REGION = "your_aws_region"
AWS_ACCESS_KEY_ID = "your_aws_access_key_id"
AWS_SECRET_ACCESS_KEY = "your_aws_secret_access_key"
STRIPE_SECRET_KEY = "your_stripe_secret_key"
LOOPS_API_KEY = "your_loops_api_key"
WORKOS_WEBHOOK_SIGNATURE = "your_workos_webhook_signature"
```

## 🚀 Running the Application

### Development Mode

```bash
# Standard run
cargo shuttle run

# With hot reload
bash scripts/watch.sh
```

### Production Deployment

```bash
cargo shuttle deploy
```

## 🧪 Testing

```bash
# Run type checks (SQLx checks run at compile time)
cargo check

# Run tests
cargo test
```

## 📚 API Endpoints

The API includes the following main endpoints:

- `/auth` - Authentication and user management
- `/chats` - Chat management and history
- `/pay` - Payment processing and subscription management
- `/oai` - AI integration endpoints
- `/sync` - Data synchronization
- `/memory` - User memory management
- `/sidekick` - Screen content analysis
- `/devents` - Device events handling

## 🏗️ Project Structure

```
src/
├── main.rs           # Application entry point
├── config.rs         # Configuration management
├── middleware/       # HTTP middleware components
├── models/           # Database models
├── prompts.rs        # AI system prompts
├── routes/           # API endpoints
└── types/            # Type definitions
```

## 🔄 Memory System

Cloak features a sophisticated memory system that:

1. Captures and stores user preferences and behaviors
2. Generates personalized memories for each user
3. Uses these memories to enhance AI responses
4. Runs scheduled jobs to maintain and update memories

## 🔐 Security

- JWT-based authentication
- Secure API key management
- CORS protection
- Request validation

## 🤝 Contributing

For internal contributors, please follow the company's development guidelines.
