use dbus::{
    arg::PropMap,
    nonblock::{Proxy, SyncConnection},
    Path,
};
use std::{fmt, sync::Arc};

use crate::{Address, Device, Result, SessionInner, SERVICE_NAME, TIMEOUT};

pub(crate) const LE_INTERFACE: &str = "org.bluez.Bearer.LE1";
pub(crate) const BREDR_INTERFACE: &str = "org.bluez.Bearer.BREDR1";

#[cfg_attr(docsrs, doc(cfg(feature = "bluetoothd")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, strum::Display, strum::EnumString)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BearerKind {
    #[strum(serialize = "le")]
    Le,
    #[strum(serialize = "bredr")]
    BrEdr,
}

impl BearerKind {
    pub(crate) const ALL: [BearerKind; 2] = [BearerKind::Le, BearerKind::BrEdr];

    pub(crate) fn interface(self) -> &'static str {
        match self {
            BearerKind::Le => LE_INTERFACE,
            BearerKind::BrEdr => BREDR_INTERFACE,
        }
    }

    pub(crate) fn from_interface(interface: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|bearer_kind| bearer_kind.interface() == interface)
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "bluetoothd")))]
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BearerProperty {
    Paired(bool),
    Bonded(bool),
    Connected(bool),
}

impl BearerProperty {
    fn from_variant_property(name: &str, value: &(dyn dbus::arg::RefArg + 'static)) -> Option<Self> {
        let flag = *dbus::arg::cast::<bool>(value)?;
        match name {
            "Paired" => Some(BearerProperty::Paired(flag)),
            "Bonded" => Some(BearerProperty::Bonded(flag)),
            "Connected" => Some(BearerProperty::Connected(flag)),
            _ => None,
        }
    }

    pub(crate) fn from_prop_map(prop_map: PropMap) -> Vec<Self> {
        prop_map.iter().filter_map(|(name, value)| Self::from_variant_property(name, &*value.0)).collect()
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "bluetoothd")))]
#[derive(Clone)]
pub struct Bearer {
    inner: Arc<SessionInner>,
    dbus_path: Path<'static>,
    adapter_name: Arc<String>,
    address: Address,
    kind: BearerKind,
}

impl fmt::Debug for Bearer {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "Bearer {{ adapter_name: {}, address: {}, kind: {} }}",
            self.adapter_name(),
            self.address(),
            self.kind()
        )
    }
}

impl Bearer {
    pub(crate) fn new(
        inner: Arc<SessionInner>, adapter_name: Arc<String>, address: Address, kind: BearerKind,
    ) -> Result<Self> {
        Ok(Self { inner, dbus_path: Device::dbus_path(&adapter_name, address)?, adapter_name, address, kind })
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, &self.dbus_path, TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn kind(&self) -> BearerKind {
        self.kind
    }

    pub async fn connect(&self) -> Result<()> {
        self.call_method_with_interface("Connect", (), self.kind.interface()).await
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.call_method_with_interface("Disconnect", (), self.kind.interface()).await
    }

    pub async fn is_paired(&self) -> Result<bool> {
        self.get_property_with_interface("Paired", self.kind.interface()).await
    }

    pub async fn is_bonded(&self) -> Result<bool> {
        self.get_property_with_interface("Bonded", self.kind.interface()).await
    }

    pub(crate) async fn is_provided(&self) -> Result<bool> {
        let connected: Option<bool> =
            self.get_opt_property_with_interface("Connected", self.kind.interface()).await?;
        Ok(connected.is_some())
    }

    pub async fn is_connected(&self) -> Result<bool> {
        self.get_property_with_interface("Connected", self.kind.interface()).await
    }

    pub async fn all_properties(&self) -> Result<Vec<BearerProperty>> {
        Ok(vec![
            BearerProperty::Paired(self.is_paired().await?),
            BearerProperty::Bonded(self.is_bonded().await?),
            BearerProperty::Connected(self.is_connected().await?),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbus::arg::{RefArg, Variant};

    fn flag_change(name: &str, flag: bool) -> PropMap {
        PropMap::from([(name.to_string(), Variant(Box::new(flag) as Box<dyn RefArg>))])
    }

    #[test]
    fn bearer_kind_round_trips_through_its_interface() {
        for bearer_kind in BearerKind::ALL {
            assert_eq!(BearerKind::from_interface(bearer_kind.interface()), Some(bearer_kind));
        }
        assert_eq!(BearerKind::from_interface("org.bluez.Device1"), None);
    }

    #[test]
    fn bearer_flags_map_to_bearer_properties() {
        assert_eq!(BearerProperty::from_prop_map(flag_change("Paired", true)), [BearerProperty::Paired(true)]);
        assert_eq!(BearerProperty::from_prop_map(flag_change("Bonded", false)), [BearerProperty::Bonded(false)]);
        assert_eq!(
            BearerProperty::from_prop_map(flag_change("Connected", false)),
            [BearerProperty::Connected(false)]
        );
    }

    #[test]
    fn unknown_bearer_property_is_skipped() {
        assert!(BearerProperty::from_prop_map(flag_change("Adapter", true)).is_empty());
    }
}
