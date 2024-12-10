numbat_wasm::imports!();

#[numbat_wasm::module]
pub trait TokenCommonModule {
    fn set_local_roles(&self, token_id: &TokenIdentifier, roles: &[DcdtLocalRole]) -> AsyncCall {
        let own_sc_address = self.blockchain().get_sc_address();

        self.send()
            .dcdt_system_sc_proxy()
            .set_special_roles(&own_sc_address, token_id, roles.into_iter().cloned())
            .async_call()
    }

    fn create_nft(&self, token_id: &TokenIdentifier, amount: &BigUint) -> u64 {
        let mut uris = ManagedVec::new();
        uris.push(ManagedBuffer::new());

        self.send().dcdt_nft_create(
            token_id,
            amount,
            &ManagedBuffer::new(),
            &BigUint::zero(),
            &ManagedBuffer::new(),
            &(),
            &uris,
        )
    }

    fn refund_owner_failed_issue(&self) {
        let owner = self.blockchain().get_owner_address();
        let rewa_returned = self.call_value().rewa_value();
        if rewa_returned > 0 {
            self.send().direct_rewa(&owner, &rewa_returned, &[]);
        }
    }
}
