use std::{ops::RangeInclusive, str::FromStr};

use hex::FromHex;
use serde::{Deserialize, Serialize};
use thiserror;
use utils::id::TenantId;

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Serialize, Deserialize, Debug, Hash)]
pub struct ShardNumber(pub u8);

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Serialize, Deserialize, Debug, Hash)]
pub struct ShardCount(pub u8);

impl ShardCount {
    pub const MAX: Self = Self(u8::MAX);
}

impl ShardNumber {
    pub const MAX: Self = Self(u8::MAX);
}

/// TenantShardId identify the units of work for the Pageserver.
///
/// These are written as `<tenant_id>-<shard number><shard-count>`, for example:
///
///   # The second shard in a two-shard tenant
///   072f1291a5310026820b2fe4b2968934-0102
///
/// Historically, tenants could not have multiple shards, and were identified
/// by TenantId.  To support this, TenantShardId has a special legacy
/// mode where `shard_count` is equal to zero: this represents a single-sharded
/// tenant which should be written as a TenantId with no suffix.
///
/// The human-readable encoding of TenantShardId, such as used in API URLs,
/// is both forward and backward compatible: a legacy TenantId can be
/// decoded as a TenantShardId, and when re-encoded it will be parseable
/// as a TenantId.
///
/// Note that the binary encoding is _not_ backward compatible, because
/// at the time sharding is introduced, there are no existing binary structures
/// containing TenantId that we need to handle.
#[derive(Eq, PartialEq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct TenantShardId {
    pub tenant_id: TenantId,
    pub shard_number: ShardNumber,
    pub shard_count: ShardCount,
}

impl TenantShardId {
    pub fn unsharded(tenant_id: TenantId) -> Self {
        Self {
            tenant_id,
            shard_number: ShardNumber(0),
            shard_count: ShardCount(0),
        }
    }

    /// The range of all TenantShardId that belong to a particular TenantId.  This is useful when
    /// you have a BTreeMap of TenantShardId, and are querying by TenantId.
    pub fn tenant_range(tenant_id: TenantId) -> RangeInclusive<Self> {
        RangeInclusive::new(
            Self {
                tenant_id,
                shard_number: ShardNumber(0),
                shard_count: ShardCount(0),
            },
            Self {
                tenant_id,
                shard_number: ShardNumber::MAX,
                shard_count: ShardCount::MAX,
            },
        )
    }

    pub fn shard_slug(&self) -> String {
        format!("{:02x}{:02x}", self.shard_number.0, self.shard_count.0)
    }
}

impl std::fmt::Display for TenantShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.shard_count != ShardCount(0) {
            write!(
                f,
                "{}-{:02x}{:02x}",
                self.tenant_id, self.shard_number.0, self.shard_count.0
            )
        } else {
            // Legacy case (shard_count == 0) -- format as just the tenant id.  Note that this
            // is distinct from the normal single shard case (shard count == 1).
            self.tenant_id.fmt(f)
        }
    }
}

impl std::fmt::Debug for TenantShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Debug is the same as Display: the compact hex representation
        write!(f, "{}", self)
    }
}

impl std::str::FromStr for TenantShardId {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Expect format: 16 byte TenantId, '-', 1 byte shard number, 1 byte shard count
        if s.len() == 32 {
            // Legacy case: no shard specified
            Ok(Self {
                tenant_id: TenantId::from_str(s)?,
                shard_number: ShardNumber(0),
                shard_count: ShardCount(0),
            })
        } else if s.len() == 37 {
            let bytes = s.as_bytes();
            let tenant_id = TenantId::from_hex(&bytes[0..32])?;
            let mut shard_parts: [u8; 2] = [0u8; 2];
            hex::decode_to_slice(&bytes[33..37], &mut shard_parts)?;
            Ok(Self {
                tenant_id,
                shard_number: ShardNumber(shard_parts[0]),
                shard_count: ShardCount(shard_parts[1]),
            })
        } else {
            Err(hex::FromHexError::InvalidStringLength)
        }
    }
}

impl From<[u8; 18]> for TenantShardId {
    fn from(b: [u8; 18]) -> Self {
        let tenant_id_bytes: [u8; 16] = b[0..16].try_into().unwrap();

        Self {
            tenant_id: TenantId::from(tenant_id_bytes),
            shard_number: ShardNumber(b[16]),
            shard_count: ShardCount(b[17]),
        }
    }
}

/// For use within the context of a particular tenant, when we need to know which
/// shard we're dealing with, but do not need to know the full ShardIdentity (because
/// we won't be doing any page->shard mapping), and do not need to know the fully qualified
/// TenantShardId.
#[derive(Eq, PartialEq, PartialOrd, Ord, Clone, Copy)]
pub struct ShardIndex {
    pub shard_number: ShardNumber,
    pub shard_count: ShardCount,
}

impl ShardIndex {
    pub fn new(number: ShardNumber, count: ShardCount) -> Self {
        Self {
            shard_number: number,
            shard_count: count,
        }
    }
    pub fn unsharded() -> Self {
        Self {
            shard_number: ShardNumber(0),
            shard_count: ShardCount(0),
        }
    }

    pub fn is_unsharded(&self) -> bool {
        self.shard_number == ShardNumber(0) && self.shard_count == ShardCount(0)
    }

    /// For use in constructing remote storage paths: concatenate this with a TenantId
    /// to get a fully qualified TenantShardId.
    ///
    /// Backward compat: this function returns an empty string if Self::is_unsharded, such
    /// that the legacy pre-sharding remote key format is preserved.
    pub fn get_suffix(&self) -> String {
        if self.is_unsharded() {
            "".to_string()
        } else {
            format!("-{:02x}{:02x}", self.shard_number.0, self.shard_count.0)
        }
    }
}

impl std::fmt::Display for ShardIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02x}{:02x}", self.shard_number.0, self.shard_count.0)
    }
}

impl std::fmt::Debug for ShardIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Debug is the same as Display: the compact hex representation
        write!(f, "{}", self)
    }
}

impl std::str::FromStr for ShardIndex {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Expect format: 1 byte shard number, 1 byte shard count
        if s.len() == 4 {
            let bytes = s.as_bytes();
            let mut shard_parts: [u8; 2] = [0u8; 2];
            hex::decode_to_slice(bytes, &mut shard_parts)?;
            Ok(Self {
                shard_number: ShardNumber(shard_parts[0]),
                shard_count: ShardCount(shard_parts[1]),
            })
        } else {
            Err(hex::FromHexError::InvalidStringLength)
        }
    }
}

impl From<[u8; 2]> for ShardIndex {
    fn from(b: [u8; 2]) -> Self {
        Self {
            shard_number: ShardNumber(b[0]),
            shard_count: ShardCount(b[1]),
        }
    }
}

impl Serialize for TenantShardId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            let mut packed: [u8; 18] = [0; 18];
            packed[0..16].clone_from_slice(&self.tenant_id.as_arr());
            packed[16] = self.shard_number.0;
            packed[17] = self.shard_count.0;

            packed.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for TenantShardId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IdVisitor {
            is_human_readable_deserializer: bool,
        }

        impl<'de> serde::de::Visitor<'de> for IdVisitor {
            type Value = TenantShardId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                if self.is_human_readable_deserializer {
                    formatter.write_str("value in form of hex string")
                } else {
                    formatter.write_str("value in form of integer array([u8; 18])")
                }
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let s = serde::de::value::SeqAccessDeserializer::new(seq);
                let id: [u8; 18] = Deserialize::deserialize(s)?;
                Ok(TenantShardId::from(id))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                TenantShardId::from_str(v).map_err(E::custom)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(IdVisitor {
                is_human_readable_deserializer: true,
            })
        } else {
            deserializer.deserialize_tuple(
                18,
                IdVisitor {
                    is_human_readable_deserializer: false,
                },
            )
        }
    }
}

/// Stripe size in number of pages
#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct ShardStripeSize(pub u32);

/// Layout version: for future upgrades where we might change how the key->shard mapping works
#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct ShardLayout(u8);

const LAYOUT_V1: ShardLayout = ShardLayout(1);

/// Default stripe size in pages: 256MiB divided by 8kiB page size.
const DEFAULT_STRIPE_SIZE: ShardStripeSize = ShardStripeSize(256 * 1024 / 8);

/// The ShardIdentity contains the information needed for one member of map
/// to resolve a key to a shard, and then check whether that shard is ==self.
#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct ShardIdentity {
    pub layout: ShardLayout,
    pub number: ShardNumber,
    pub count: ShardCount,
    pub stripe_size: ShardStripeSize,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ShardConfigError {
    #[error("Invalid shard count")]
    InvalidCount,
    #[error("Invalid shard number")]
    InvalidNumber,
    #[error("Invalid stripe size")]
    InvalidStripeSize,
}

impl ShardIdentity {
    /// An identity with number=0 count=0 is a "none" identity, which represents legacy
    /// tenants.  Modern single-shard tenants should not use this: they should
    /// have number=0 count=1.
    pub fn unsharded() -> Self {
        Self {
            number: ShardNumber(0),
            count: ShardCount(0),
            layout: LAYOUT_V1,
            stripe_size: DEFAULT_STRIPE_SIZE,
        }
    }

    pub fn is_unsharded(&self) -> bool {
        self.number == ShardNumber(0) && self.count == ShardCount(0)
    }

    /// Count must be nonzero, and number must be < count. To construct
    /// the legacy case (count==0), use Self::unsharded instead.
    pub fn new(
        number: ShardNumber,
        count: ShardCount,
        stripe_size: ShardStripeSize,
    ) -> Result<Self, ShardConfigError> {
        if count.0 == 0 {
            Err(ShardConfigError::InvalidCount)
        } else if number.0 > count.0 - 1 {
            Err(ShardConfigError::InvalidNumber)
        } else if stripe_size.0 == 0 {
            Err(ShardConfigError::InvalidStripeSize)
        } else {
            Ok(Self {
                number,
                count,
                layout: LAYOUT_V1,
                stripe_size,
            })
        }
    }
}

impl Serialize for ShardIndex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            // Binary encoding is not used in index_part.json, but is included in anticipation of
            // switching various structures (e.g. inter-process communication, remote metadata) to more
            // compact binary encodings in future.
            let mut packed: [u8; 2] = [0; 2];
            packed[0] = self.shard_number.0;
            packed[1] = self.shard_count.0;
            packed.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for ShardIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IdVisitor {
            is_human_readable_deserializer: bool,
        }

        impl<'de> serde::de::Visitor<'de> for IdVisitor {
            type Value = ShardIndex;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                if self.is_human_readable_deserializer {
                    formatter.write_str("value in form of hex string")
                } else {
                    formatter.write_str("value in form of integer array([u8; 2])")
                }
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let s = serde::de::value::SeqAccessDeserializer::new(seq);
                let id: [u8; 2] = Deserialize::deserialize(s)?;
                Ok(ShardIndex::from(id))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ShardIndex::from_str(v).map_err(E::custom)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(IdVisitor {
                is_human_readable_deserializer: true,
            })
        } else {
            deserializer.deserialize_tuple(
                2,
                IdVisitor {
                    is_human_readable_deserializer: false,
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bincode;
    use utils::{id::TenantId, Hex};

    use super::*;

    const EXAMPLE_TENANT_ID: &str = "1f359dd625e519a1a4e8d7509690f6fc";

    #[test]
    fn tenant_shard_id_string() -> Result<(), hex::FromHexError> {
        let example = TenantShardId {
            tenant_id: TenantId::from_str(EXAMPLE_TENANT_ID).unwrap(),
            shard_count: ShardCount(10),
            shard_number: ShardNumber(7),
        };

        let encoded = format!("{example}");

        let expected = format!("{EXAMPLE_TENANT_ID}-070a");
        assert_eq!(&encoded, &expected);

        let decoded = TenantShardId::from_str(&encoded)?;

        assert_eq!(example, decoded);

        Ok(())
    }

    #[test]
    fn tenant_shard_id_binary() -> Result<(), hex::FromHexError> {
        let example = TenantShardId {
            tenant_id: TenantId::from_str(EXAMPLE_TENANT_ID).unwrap(),
            shard_count: ShardCount(10),
            shard_number: ShardNumber(7),
        };

        let encoded = bincode::serialize(&example).unwrap();
        let expected: [u8; 18] = [
            0x1f, 0x35, 0x9d, 0xd6, 0x25, 0xe5, 0x19, 0xa1, 0xa4, 0xe8, 0xd7, 0x50, 0x96, 0x90,
            0xf6, 0xfc, 0x07, 0x0a,
        ];
        assert_eq!(Hex(&encoded), Hex(&expected));

        let decoded = bincode::deserialize(&encoded).unwrap();

        assert_eq!(example, decoded);

        Ok(())
    }

    #[test]
    fn tenant_shard_id_backward_compat() -> Result<(), hex::FromHexError> {
        // Test that TenantShardId can decode a TenantId in human
        // readable form
        let example = TenantId::from_str(EXAMPLE_TENANT_ID).unwrap();
        let encoded = format!("{example}");

        assert_eq!(&encoded, EXAMPLE_TENANT_ID);

        let decoded = TenantShardId::from_str(&encoded)?;

        assert_eq!(example, decoded.tenant_id);
        assert_eq!(decoded.shard_count, ShardCount(0));
        assert_eq!(decoded.shard_number, ShardNumber(0));

        Ok(())
    }

    #[test]
    fn tenant_shard_id_forward_compat() -> Result<(), hex::FromHexError> {
        // Test that a legacy TenantShardId encodes into a form that
        // can be decoded as TenantId
        let example_tenant_id = TenantId::from_str(EXAMPLE_TENANT_ID).unwrap();
        let example = TenantShardId::unsharded(example_tenant_id);
        let encoded = format!("{example}");

        assert_eq!(&encoded, EXAMPLE_TENANT_ID);

        let decoded = TenantId::from_str(&encoded)?;

        assert_eq!(example_tenant_id, decoded);

        Ok(())
    }

    #[test]
    fn tenant_shard_id_legacy_binary() -> Result<(), hex::FromHexError> {
        // Unlike in human readable encoding, binary encoding does not
        // do any special handling of legacy unsharded TenantIds: this test
        // is equivalent to the main test for binary encoding, just verifying
        // that the same behavior applies when we have used `unsharded()` to
        // construct a TenantShardId.
        let example = TenantShardId::unsharded(TenantId::from_str(EXAMPLE_TENANT_ID).unwrap());
        let encoded = bincode::serialize(&example).unwrap();

        let expected: [u8; 18] = [
            0x1f, 0x35, 0x9d, 0xd6, 0x25, 0xe5, 0x19, 0xa1, 0xa4, 0xe8, 0xd7, 0x50, 0x96, 0x90,
            0xf6, 0xfc, 0x00, 0x00,
        ];
        assert_eq!(Hex(&encoded), Hex(&expected));

        let decoded = bincode::deserialize::<TenantShardId>(&encoded).unwrap();
        assert_eq!(example, decoded);

        Ok(())
    }

    #[test]
    fn shard_identity_validation() -> Result<(), ShardConfigError> {
        // Happy cases
        ShardIdentity::new(ShardNumber(0), ShardCount(1), DEFAULT_STRIPE_SIZE)?;
        ShardIdentity::new(ShardNumber(0), ShardCount(1), ShardStripeSize(1))?;
        ShardIdentity::new(ShardNumber(254), ShardCount(255), ShardStripeSize(1))?;

        assert_eq!(
            ShardIdentity::new(ShardNumber(0), ShardCount(0), DEFAULT_STRIPE_SIZE),
            Err(ShardConfigError::InvalidCount)
        );
        assert_eq!(
            ShardIdentity::new(ShardNumber(10), ShardCount(10), DEFAULT_STRIPE_SIZE),
            Err(ShardConfigError::InvalidNumber)
        );
        assert_eq!(
            ShardIdentity::new(ShardNumber(11), ShardCount(10), DEFAULT_STRIPE_SIZE),
            Err(ShardConfigError::InvalidNumber)
        );
        assert_eq!(
            ShardIdentity::new(ShardNumber(255), ShardCount(255), DEFAULT_STRIPE_SIZE),
            Err(ShardConfigError::InvalidNumber)
        );
        assert_eq!(
            ShardIdentity::new(ShardNumber(0), ShardCount(1), ShardStripeSize(0)),
            Err(ShardConfigError::InvalidStripeSize)
        );

        Ok(())
    }

    #[test]
    fn shard_index_human_encoding() -> Result<(), hex::FromHexError> {
        let example = ShardIndex {
            shard_number: ShardNumber(13),
            shard_count: ShardCount(17),
        };
        let expected: String = "0d11".to_string();
        let encoded = format!("{example}");
        assert_eq!(&encoded, &expected);

        let decoded = ShardIndex::from_str(&encoded)?;
        assert_eq!(example, decoded);
        Ok(())
    }

    #[test]
    fn shard_index_binary_encoding() -> Result<(), hex::FromHexError> {
        let example = ShardIndex {
            shard_number: ShardNumber(13),
            shard_count: ShardCount(17),
        };
        let expected: [u8; 2] = [0x0d, 0x11];

        let encoded = bincode::serialize(&example).unwrap();
        assert_eq!(Hex(&encoded), Hex(&expected));
        let decoded = bincode::deserialize(&encoded).unwrap();
        assert_eq!(example, decoded);

        Ok(())
    }
}
