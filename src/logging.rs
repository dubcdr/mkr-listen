pub use logging::*;

mod logging {
    use std::collections::HashMap;

    use ethers::prelude::*;
    use paris::Logger;
    use token_list::Token;

    use crate::{uni_helpers::UniTxnInputs, uni_v2_router::UniV2Router};

    pub fn log_txns<T>(
        txns: Vec<&Transaction>,
        token_map: &HashMap<String, Token>,
        uni_router_contract: &UniV2Router<Provider<T>>,
    ) where
        T: JsonRpcClient,
    {
        let mut logger = Logger::new();
        let call_datas: Vec<(&Transaction, UniTxnInputs)> = txns
            .iter()
            .map(|txn| {
                let call_data = UniTxnInputs::new(&txn, &uni_router_contract);
                (*txn, call_data)
            })
            .collect();
        call_datas.iter().for_each(|(txn, call_data)| {
            logger.indent(1).log(format!(
                "{} :: {}",
                log_txn(txn),
                log_swap_inputs(call_data, token_map)
            ));
        })
    }

    fn log_txn(txn: &Transaction) -> String {
        format!("Txn {}", txn.hash)
    }

    fn log_swap_inputs(call_data: &UniTxnInputs, token_map: &HashMap<String, Token>) -> String {
        call_data.log_str(token_map)
    }
}
