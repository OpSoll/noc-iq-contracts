# NOCIQ Smart Contracts

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Soroban](https://img.shields.io/badge/Soroban-Compatible-7D00FF)](https://soroban.stellar.org/)
[![Rust](https://img.shields.io/badge/Rust-1.74+-orange?logo=rust)](https://www.rust-lang.org/)

> Soroban smart contracts for automated SLA management and payment processing on the Stellar blockchain

## 🌟 Overview

This repository contains the Soroban smart contracts that power NOCIQ's blockchain-based SLA enforcement system. These contracts enable automated penalty/reward calculations and transparent, tamper-proof payment execution based on network outage resolution times.

**Parent Project:** [NOCIQ](https://github.com/OpSoll/noc-iq-fe)  
**Frontend:** [noc-iq-fe](https://github.com/OpSoll/noc-iq-fe)  
**Backend:** [noc-iq-be](https://github.com/OpSoll/noc-iq-be)

## 📋 Contracts

### 1. SLA Calculator (`sla_calculator/`)
Calculates penalties and rewards based on Mean Time To Repair (MTTR) and predefined SLA thresholds.

**Features:**
- Automated penalty calculation for SLA violations
- Reward calculation for early resolution
- Configurable thresholds per severity level
- Admin-controlled configuration updates
- Transparent calculation logic

### 2. Payment Escrow (`payment_escrow/`)
Manages conditional payment releases with escrow functionality.

**Features:**
- Lock funds until conditions are met
- Multi-party approval mechanisms
- Automatic release on condition fulfillment
- Refund capability for failed conditions

### 3. Multi-Party Settlement (`multi_party_settlement/`)
Handles cost-sharing between multiple network operators for shared infrastructure incidents.

**Features:**
- Automatic cost splitting based on usage
- Proportional allocation algorithms
- Batch settlement processing
- Audit trail for all distributions

## 🛠️ Technology Stack

- **Language:** Rust
- **Framework:** Soroban SDK
- **Blockchain:** Stellar Network
- **Testing:** Rust `cargo test`
- **Deployment:** Soroban CLI

## 🚀 Quick Start

### Prerequisites

- **Rust** 1.74 or higher ([Install](https://rustup.rs/))
- **Soroban CLI** ([Install](https://soroban.stellar.org/docs/getting-started/setup))
- **Stellar CLI** (optional, for account management)

### Installation

```bash
# Clone the repository
git clone https://github.com/OpSoll/noc-iq-contracts.git
cd noc-iq-contracts

# Install Soroban CLI
cargo install --locked soroban-cli

# Build all contracts
make build

# Run tests
make test
```

## 📁 Project Structure

```
noc-iq-contracts/
├── sla_calculator/           # SLA calculation contract
│   ├── src/
│   │   ├── lib.rs           # Main contract logic
│   │   ├── types.rs         # Data structures
│   │   └── tests.rs         # Unit tests
│   └── Cargo.toml
├── payment_escrow/           # Payment escrow contract
│   ├── src/
│   │   ├── lib.rs
│   │   └── tests.rs
│   └── Cargo.toml
├── multi_party_settlement/   # Multi-party settlement
│   ├── src/
│   │   ├── lib.rs
│   │   └── tests.rs
│   └── Cargo.toml
├── scripts/                  # Deployment scripts
│   ├── deploy.sh
│   ├── initialize.sh
│   └── test-invoke.sh
├── docs/                     # Documentation
│   ├── SLA_CALCULATOR.md
│   ├── PAYMENT_ESCROW.md
│   └── DEPLOYMENT.md
├── Makefile                  # Build automation
├── Cargo.toml               # Workspace configuration
└── README.md
```

## 🔨 Building Contracts

### Build All Contracts

```bash
make build
```

This compiles all contracts to WebAssembly (WASM) and places them in `target/wasm32-unknown-unknown/release/`.

### Build Individual Contract

```bash
cd sla_calculator
cargo build --target wasm32-unknown-unknown --release
```

### Optimize WASM (Production)

```bash
make optimize
```

Uses `wasm-opt` to reduce contract size and gas costs.

## 🧪 Testing

### Run All Tests

```bash
make test
```

### Run Tests for Specific Contract

```bash
cd sla_calculator
cargo test
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

### Test Coverage

```bash
cargo tarpaulin --out Html
```

## 🚀 Deployment

### Deploy to Testnet

```bash
# Deploy SLA Calculator
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/sla_calculator.wasm \
  --source-account testnet-deployer \
  --network testnet

# Save the contract ID
export SLA_CONTRACT_ID=CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC
```

### Initialize Contract

```bash
soroban contract invoke \
  --id $SLA_CONTRACT_ID \
  --source-account testnet-deployer \
  --network testnet \
  -- initialize \
  --admin GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX \
  --usdc_token CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB \
  --pool_address GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
```

### Using Deployment Scripts

```bash
# Deploy all contracts to testnet
./scripts/deploy.sh testnet

# Initialize contracts
./scripts/initialize.sh testnet

# Test contract invocation
./scripts/test-invoke.sh testnet
```

## 📚 Contract Documentation

### SLA Calculator

#### Functions

**`initialize(admin: Address, usdc_token: Address, pool_address: Address)`**
- Initializes the contract with admin and token addresses
- Can only be called once
- Sets default SLA configurations

**`calculate_sla(outage_id: Symbol, severity: Severity, mttr_minutes: u32) -> SLAResult`**
- Calculates penalty or reward based on MTTR
- Returns status, amount, and payment type
- Pure function (doesn't modify state)

**`execute_payment(sla_result: SLAResult, operator_address: Address, noc_team_address: Address) -> PaymentRecord`**
- Executes payment based on SLA result
- Requires authorization from payer
- Stores payment record on-chain

**`get_config(severity: Severity) -> SLAConfig`**
- Retrieves SLA configuration for severity level
- Public read-only function

**`update_config(severity: Severity, threshold_minutes: u32, penalty_per_minute: i128, reward_base: i128)`**
- Updates SLA configuration (admin only)
- Requires admin authorization

#### Data Structures

```rust
pub enum Severity {
    Critical = 1,
    High = 2,
    Medium = 3,
    Low = 4,
}

pub struct SLAConfig {
    pub threshold_minutes: u32,
    pub penalty_per_minute: i128,
    pub reward_base: i128,
}

pub struct SLAResult {
    pub outage_id: Symbol,
    pub status: Symbol,        // "met" or "violated"
    pub mttr_minutes: u32,
    pub threshold_minutes: u32,
    pub amount: i128,          // Negative for penalty, positive for reward
    pub payment_type: Symbol,  // "reward" or "penalty"
    pub rating: Symbol,        // "exceptional", "excellent", "good", "poor"
}
```

#### Usage Example

```rust
// Calculate SLA for a critical outage resolved in 25 minutes
let result = client.calculate_sla(
    &Symbol::short("OUT001"),
    &Severity::Critical,
    &25  // MTTR: 25 minutes (threshold is 15 minutes)
);

// Result:
// status: "violated"
// amount: -1000_0000000 (negative = penalty of $1,000)
// payment_type: "penalty"
```

### Payment Escrow

#### Functions

**`create_escrow(amount: i128, conditions: Vec<Condition>) -> EscrowId`**
- Creates new escrow with specified conditions
- Locks funds until conditions are met
- Returns unique escrow ID

**`release_escrow(escrow_id: EscrowId)`**
- Releases funds when all conditions are satisfied
- Transfers to designated recipient
- Can only be called by authorized parties

**`refund_escrow(escrow_id: EscrowId)`**
- Refunds locked funds to sender
- Only possible if conditions failed or timeout reached
- Requires appropriate authorization

### Multi-Party Settlement

#### Functions

**`create_settlement(incident_id: Symbol, total_cost: i128, parties: Vec<Party>)`**
- Creates cost-sharing agreement
- Defines proportions for each party
- Validates that proportions sum to 100%

**`execute_settlement(incident_id: Symbol)`**
- Executes payments to all parties
- Uses predefined proportions
- Records all transactions

## 🔐 Security

### Auditing

- All contracts should be audited before mainnet deployment
- We recommend using [Halborn](https://halborn.com/) or [Trail of Bits](https://www.trailofbits.com/)
- Test thoroughly on testnet with real scenarios

### Key Management

- Never commit private keys to repository
- Use hardware wallets for mainnet deployment
- Implement multi-sig for admin functions in production

### Access Control

- Admin functions restricted to designated addresses
- Payment functions require proper authorization
- All sensitive operations emit events for monitoring

## 🧪 Testing Strategy

### Unit Tests
- Test each function in isolation
- Cover all edge cases
- Test error conditions

### Integration Tests
- Test contract interactions
- Verify state changes
- Test against actual Stellar testnet

### Scenario Tests
- Simulate real-world outage scenarios
- Test with various MTTR values
- Verify payment calculations

### Example Test

```rust
#[test]
fn test_sla_violation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLAContract);
    let client = SLAContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let usdc = Address::generate(&env);
    let pool = Address::generate(&env);
    
    client.initialize(&admin, &usdc, &pool);
    
    // Critical outage resolved in 25 minutes (threshold: 15)
    let result = client.calculate_sla(
        &Symbol::short("OUT001"),
        &Severity::Critical,
        &25
    );
    
    assert_eq!(result.status, Symbol::short("violated"));
    assert_eq!(result.amount, -1000_0000000); // $1,000 penalty
}
```

## 📊 Gas Optimization

### Best Practices

- Minimize storage operations
- Use efficient data structures
- Batch operations when possible
- Cache frequently accessed data

### Typical Gas Costs

| Operation | Gas Cost (XLM) |
|-----------|----------------|
| Deploy contract | ~0.1 - 0.5 |
| Initialize | ~0.01 |
| Calculate SLA | ~0.001 |
| Execute payment | ~0.005 |
| Update config | ~0.002 |

**Note:** Actual costs vary based on network congestion.

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md).

### Development Workflow

1. Fork the repository
2. Create feature branch: `git checkout -b feature/new-contract`
3. Write tests first
4. Implement functionality
5. Run tests: `cargo test`
6. Run clippy: `cargo clippy -- -D warnings`
7. Format code: `cargo fmt`
8. Submit pull request

### Contract Development Guidelines

- Follow Rust best practices
- Write comprehensive tests (aim for 100% coverage)
- Document all public functions
- Use descriptive error messages
- Consider gas costs in design
- Emit events for important state changes

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Soroban SDK](https://soroban.stellar.org/)
- Powered by [Stellar Network](https://stellar.org/)
- Inspired by DeFi protocols and traditional SLA contracts

## 📧 Contact & Support

- **Issues**: [GitHub Issues](https://github.com/OpSoll/noc-iq-contracts/issues)
- **Discussions**: [GitHub Discussions](https://github.com/OpSoll/noc-iq-contracts/discussions)
- **Stellar Discord**: [Join](https://discord.gg/stellardev)
- **Soroban Discord**: [Join](https://discord.gg/soroban)

## 🗺️ Roadmap

- [ ] SLA Calculator contract
- [ ] Comprehensive test suite
- [ ] Documentation
- [ ] Payment Escrow contract
- [ ] Multi-Party Settlement contract
- [ ] Security audit
- [ ] Mainnet deployment
- [ ] Governance features
- [ ] Upgrade mechanisms
- [ ] Advanced analytics contracts

---

**Made with ❤️ by the OpSoll Team | Powered by Stellar ⭐**

**Building on Stellar? Join us in the [Stellar Wave Program](https://www.drips.network/wave/stellar)!**
