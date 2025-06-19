use parking_lot_core::UnparkToken;

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
pub(crate) const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
pub(crate) const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);
