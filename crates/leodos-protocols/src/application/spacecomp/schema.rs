use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Ref;
use zerocopy::Unaligned;

/// A zero-copy serializable data type with a key for partitioning.
pub trait Schema: IntoBytes + FromBytes + KnownLayout + Immutable + Unaligned {
    /// The key type used for partitioning data across mappers.
    type Key<'a>;
    /// Extracts the partition key from a zero-copy reference.
    fn key<'a>(pkt: &Ref<&'a [u8], Self>) -> Self::Key<'a>;
}

#[cfg(test)]
mod test {
    use zerocopy::network_endian::F32;
    use zerocopy::network_endian::U32;
    use zerocopy::FromBytes;
    use zerocopy::Immutable;
    use zerocopy::IntoBytes;
    use zerocopy::KnownLayout;
    use zerocopy::Ref;
    use zerocopy::Unaligned;

    use super::Schema;
    use crate::network::spp::Apid;
    use crate::network::spp::SpacePacket;

    impl Schema for SpacePacket {
        type Key<'a> = Apid;

        fn key<'a>(data: &Ref<&'a [u8], Self>) -> Self::Key<'a> {
            data.apid()
        }
    }

    #[repr(C)]
    #[derive(Debug, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
    pub struct NamedPacket {
        pub name_buf: [u8; 16],
        pub value: U32,
    }

    impl Schema for NamedPacket {
        type Key<'a> = &'a str;

        fn key<'a>(data: &Ref<&'a [u8], NamedPacket>) -> &'a str {
            let packet: &NamedPacket = Ref::into_ref(*data);
            let slice = &packet.name_buf;

            core::str::from_utf8(slice)
                .unwrap_or("INVALID")
                .trim_end_matches('\0')
        }
    }

    // Sensor example
    #[repr(C)]
    #[derive(Debug, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
    pub struct TempReading {
        pub sensor_index: u8,
        pub celsius: F32,
    }

    impl Schema for TempReading {
        type Key<'a> = u8;

        fn key<'a>(data: &Ref<&'a [u8], Self>) -> Self::Key<'a> {
            let packet: &TempReading = Ref::into_ref(*data);
            packet.sensor_index
        }
    }
}
