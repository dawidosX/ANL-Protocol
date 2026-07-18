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
