use crate::id::TenantId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceOwner {
    pub tenant_id: TenantId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SiteKind {
    Php,
    Static,
    ReverseProxy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_kind_serde() {
        let kind = SiteKind::Php;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"php\"");

        let kind = SiteKind::Static;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"static\"");

        let kind = SiteKind::ReverseProxy;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"reverseProxy\"");
    }
}
