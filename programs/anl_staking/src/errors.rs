use anchor_lang::prelude::*;

#[error_code]
pub enum AnlError {
    #[msg("Protocol is paused")]
    Paused,
    #[msg("Pool is paused")]
    PoolPaused,
    #[msg("Pool is closed")]
    PoolClosed,
    #[msg("Protocol is already paused")]
    AlreadyPaused,
    #[msg("Protocol is not paused")]
    NotPaused,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Amount below minimum stake")]
    BelowMinimumStake,
    #[msg("Amount overflow")]
    AmountOverflow,
    #[msg("Declared period out of bounds (7..=3650 days)")]
    InvalidPeriod,
    #[msg("Position period has already ended - use claim")]
    PeriodAlreadyEnded,
    #[msg("Genesis positions are locked until period end - no early exit (WP v1.1)")]
    GenesisLocked,
    #[msg("Reward period has not ended")]
    PeriodNotEnded,
    #[msg("Nothing to claim")]
    NothingToClaim,
    #[msg("Claim matured rewards before unstaking")]
    ClaimFirst,
    #[msg("Insufficient reward vault balance")]
    InsufficientRewardVault,
    #[msg("Insufficient XNT vault balance")]
    InsufficientXntVault,
    #[msg("Invalid mint")]
    InvalidMint,
    #[msg("Invalid token program for this mint")]
    InvalidTokenProgram,
    #[msg("Invalid vault account")]
    InvalidVault,
    #[msg("Invalid authority")]
    InvalidAuthority,
    #[msg("Position owner mismatch")]
    PositionOwnerMismatch,
    #[msg("Position is closed")]
    PositionClosed,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Division by zero")]
    DivisionByZero,
    #[msg("Negative or invalid time")]
    InvalidTime,
    #[msg("Pool has active positions")]
    PoolNotEmpty,
    #[msg("Pool has pending obligations")]
    PendingObligations,
    #[msg("Unsupported account version")]
    InvalidAccountVersion,
    #[msg("Genesis start must not be in the past")]
    GenesisStartInPast,
    #[msg("Staking has not started yet")]
    NotStarted,
    #[msg("Reward pool cannot cover this position - stake rejected")]
    RewardCoverageExceeded,
    #[msg("Position already settled")]
    AlreadySettled,
    #[msg("ANL mint has a forbidden Token-2022 extension")]
    ForbiddenMintExtension,
    #[msg("ANL mint must not have a freeze authority")]
    MintHasFreezeAuthority,
    #[msg("ANL mint must not have a mint authority (fixed supply required)")]
    MintHasMintAuthority,
    #[msg("XNT mint does not match the expected wrapped-native mint")]
    InvalidXntMint,
    #[msg("Funding epoch does not match the current clock epoch")]
    EpochMismatch,
    #[msg("Timestamp precedes protocol genesis")]
    BeforeGenesis,
    #[msg("Previous-epoch checkpoint account is required")]
    CheckpointRequired,
    #[msg("Checkpoint account does not match the expected PDA/epoch")]
    CheckpointMismatch,
    #[msg("Operator pubkey is invalid or unchanged")]
    InvalidOperator,
    #[msg("Genesis window not reached yet (need a full 30-day block)")]
    WindowNotReached,
    #[msg("Genesis window claim is only for Genesis positions")]
    NotGenesisPool,
}

impl From<anl_math::MathError> for AnlError {
    fn from(e: anl_math::MathError) -> Self {
        match e {
            anl_math::MathError::Overflow => AnlError::MathOverflow,
            anl_math::MathError::DivisionByZero => AnlError::DivisionByZero,
            anl_math::MathError::NegativeTime => AnlError::InvalidTime,
            anl_math::MathError::IndexUnderflow => AnlError::MathOverflow,
        }
    }
}
