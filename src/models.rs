#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HashId([u8; Self::LEN]);

impl HashId {
    /// The number of bytes in a hash.
    const LEN: usize = 32;

    /// The length (in bytes) of a hexadecimal representation of a hash.
    const STR_LEN: usize = Self::LEN * 2;
}

impl std::fmt::Display for HashId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut hex = [0u8; 64];
        hex::encode_to_slice(&self.0, &mut hex).expect("hex::encode_to_slice failed");
        let hex = std::str::from_utf8(&hex).expect("invalid UTF-8");

        f.write_str(hex)
    }
}

impl std::fmt::Debug for HashId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut hex = [0u8; Self::STR_LEN];
        hex::encode_to_slice(&self.0, &mut hex).expect("hex::encode_to_slice failed");
        let hex = std::str::from_utf8(&hex).expect("invalid UTF-8");
        f.debug_tuple("BlobId").field(&hex).finish()
    }
}

impl std::str::FromStr for HashId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        anyhow::ensure!(
            s.len() == Self::STR_LEN,
            "expected hash to be {} bytes long, was {}",
            Self::STR_LEN,
            s.len()
        );

        let mut bytes = [0u8; Self::LEN];
        hex::decode_to_slice(&s, &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl serde::Serialize for HashId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for HashId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        let hash = hex.parse().map_err(serde::de::Error::custom)?;
        Ok(hash)
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSource {
    pub artifact_hash: HashId,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BakeOutput {
    pub output_hash: HashId,
}
