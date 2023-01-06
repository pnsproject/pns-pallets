use core::str::FromStr;

use codec::{Decode, Encode};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use trust_dns_proto::rr::{Name, RData, RecordType};
use trust_dns_proto::{rr::rdata::NULL, serialize::binary::BinDecodable};

#[cfg(feature = "std")]
#[derive(thiserror::Error)]
pub enum Error {
    #[error("Try from chain data failed.")]
    TryFromError,
    #[error("Try from chain str data failed.")]
    FromStrError,
    #[error("Try from chain utf-8 data failed.")]
    FromUtf8Error,
}

#[cfg(feature = "std")]
impl TryFrom<codec_type::RData> for RData {
    type Error = Error;
    fn try_from(value: codec_type::RData) -> Result<Self, Self::Error> {
        match value {
            codec_type::RData::A(codec_type::Ipv4Addr { octets }) => {
                Ok(RData::A(std::net::Ipv4Addr::from(octets)))
            }
            codec_type::RData::AAAA(codec_type::Ipv6Addr { octets }) => {
                Ok(RData::AAAA(std::net::Ipv6Addr::from(octets)))
            }
            codec_type::RData::ANAME(name) => Name::try_from(name).map(RData::ANAME),
            codec_type::RData::CAA(_) => todo!(),
            codec_type::RData::CNAME(name) => Name::try_from(name).map(RData::CNAME),
            codec_type::RData::CSYNC(_) => todo!(),
            codec_type::RData::HINFO(_) => todo!(),
            codec_type::RData::HTTPS(_) => todo!(),
            codec_type::RData::MX(_) => todo!(),
            codec_type::RData::NAPTR(_) => todo!(),
            codec_type::RData::NULL(codec_type::NULL { anything }) => {
                Ok(RData::NULL(NULL::with(anything)))
            }
            codec_type::RData::NS(name) => Name::try_from(name).map(RData::NS),
            codec_type::RData::OPENPGPKEY(_) => todo!(),
            codec_type::RData::OPT(_) => todo!(),
            codec_type::RData::PTR(name) => Name::try_from(name).map(RData::PTR),
            codec_type::RData::SOA(_) => todo!(),
            codec_type::RData::SRV(_) => todo!(),
            codec_type::RData::SSHFP(_) => todo!(),
            codec_type::RData::SVCB(_) => todo!(),
            codec_type::RData::TLSA(_) => todo!(),
            codec_type::RData::TXT(_) => todo!(),
            codec_type::RData::DNSSEC(_) => todo!(),
            codec_type::RData::Unknown { code, rdata } => Ok(RData::Unknown {
                code,
                rdata: NULL::with(rdata.anything),
            }),
            codec_type::RData::ZERO => Ok(RData::ZERO),
        }
    }
}

#[cfg(feature = "std")]
impl TryFrom<codec_type::Name> for Name {
    type Error = Error;

    fn try_from(codec_type::Name(name): codec_type::Name) -> Result<Self, Self::Error> {
        let name = Name::from_str(&String::from_utf8(name).map_err(|_| Error::FromUtf8Error)?)
            .map_err(|_| Error::FromStrError)?;
        Ok(name)
    }
}

#[cfg(feature = "std")]
impl From<codec_type::RecordType> for RecordType {
    fn from(value: codec_type::RecordType) -> Self {
        match value {
            codec_type::RecordType::A => RecordType::A,
            codec_type::RecordType::AAAA => RecordType::AAAA,
            codec_type::RecordType::ANAME => RecordType::ANAME,
            codec_type::RecordType::ANY => RecordType::ANY,
            codec_type::RecordType::AXFR => RecordType::AXFR,
            codec_type::RecordType::CAA => RecordType::CAA,
            codec_type::RecordType::CDS => RecordType::CDS,
            codec_type::RecordType::CDNSKEY => RecordType::CDNSKEY,
            codec_type::RecordType::CNAME => RecordType::CNAME,
            codec_type::RecordType::CSYNC => RecordType::CSYNC,
            codec_type::RecordType::DNSKEY => RecordType::DNSKEY,
            codec_type::RecordType::DS => RecordType::DS,
            codec_type::RecordType::HINFO => RecordType::HINFO,
            codec_type::RecordType::HTTPS => RecordType::HTTPS,
            codec_type::RecordType::IXFR => RecordType::IXFR,
            codec_type::RecordType::KEY => RecordType::KEY,
            codec_type::RecordType::MX => RecordType::MX,
            codec_type::RecordType::NAPTR => RecordType::NAPTR,
            codec_type::RecordType::NS => RecordType::NS,
            codec_type::RecordType::NSEC => RecordType::NSEC,
            codec_type::RecordType::NSEC3 => RecordType::NSEC3,
            codec_type::RecordType::NSEC3PARAM => RecordType::NSEC3PARAM,
            codec_type::RecordType::NULL => RecordType::NULL,
            codec_type::RecordType::OPENPGPKEY => RecordType::OPENPGPKEY,
            codec_type::RecordType::OPT => RecordType::OPT,
            codec_type::RecordType::PTR => RecordType::PTR,
            codec_type::RecordType::RRSIG => RecordType::RRSIG,
            codec_type::RecordType::SIG => RecordType::SIG,
            codec_type::RecordType::SOA => RecordType::SOA,
            codec_type::RecordType::SRV => RecordType::SRV,
            codec_type::RecordType::SSHFP => RecordType::SSHFP,
            codec_type::RecordType::SVCB => RecordType::SVCB,
            codec_type::RecordType::TLSA => RecordType::TLSA,
            codec_type::RecordType::TSIG => RecordType::TSIG,
            codec_type::RecordType::TXT => RecordType::TXT,
            codec_type::RecordType::Unknown(unknow) => RecordType::Unknown(unknow),
            codec_type::RecordType::ZERO => RecordType::ZERO,
        }
    }
}

pub mod codec_type {
    use core::mem;

    use hashbrown::HashMap;

    use codec::{Compact, Decode, Encode, Error, MaxEncodedLen};
    use scale_info::{build::Fields, Type, TypeInfo};
    #[cfg(feature = "std")]
    use serde::{Deserialize, Serialize};
    use sp_std::fmt::Debug;
    use sp_std::vec::Vec;

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[allow(dead_code)]
    #[non_exhaustive]
    pub enum RecordType {
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) IPv4 Address record
        A,
        /// [RFC 3596](https://tools.ietf.org/html/rfc3596) IPv6 address record
        AAAA,
        /// [ANAME draft-ietf-dnsop-aname](https://tools.ietf.org/html/draft-ietf-dnsop-aname-04)
        ANAME,
        //  AFSDB,      //	18	RFC 1183	AFS database record
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) All cached records, aka ANY
        ANY,
        //  APL,        //	42	RFC 3123	Address Prefix List
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) Authoritative Zone Transfer
        AXFR,
        /// [RFC 6844](https://tools.ietf.org/html/rfc6844) Certification Authority Authorization
        CAA,
        /// [RFC 7344](https://tools.ietf.org/html/rfc7344) Child DS
        CDS,
        /// [RFC 7344](https://tools.ietf.org/html/rfc7344) Child DNSKEY
        CDNSKEY,
        //  CERT,       // 37 RFC 4398 Certificate record
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) Canonical name record
        CNAME,
        //  DHCID,      // 49 RFC 4701 DHCP identifier
        //  DLV,        //	32769	RFC 4431	DNSSEC Lookaside Validation record
        //  DNAME,      // 39 RFC 2672 Delegation Name
        /// [RFC 7477](https://tools.ietf.org/html/rfc4034) Child-to-parent synchronization record
        CSYNC,
        /// [RFC 4034](https://tools.ietf.org/html/rfc4034) DNS Key record: RSASHA256 and RSASHA512, RFC5702
        DNSKEY,
        /// [RFC 4034](https://tools.ietf.org/html/rfc4034) Delegation signer: RSASHA256 and RSASHA512, RFC5702
        DS,
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) host information
        HINFO,
        //  HIP,        // 55 RFC 5205 Host Identity Protocol
        /// [RFC draft-ietf-dnsop-svcb-https-03](https://tools.ietf.org/html/draft-ietf-dnsop-svcb-httpssvc-03) DNS SVCB and HTTPS RRs
        HTTPS,
        //  IPSECKEY,   // 45 RFC 4025 IPsec Key
        /// [RFC 1996](https://tools.ietf.org/html/rfc1996) Incremental Zone Transfer
        IXFR,
        //  KX,         // 36 RFC 2230 Key eXchanger record
        /// [RFC 2535](https://tools.ietf.org/html/rfc2535) and [RFC 2930](https://tools.ietf.org/html/rfc2930) Key record
        KEY,
        //  LOC,        // 29 RFC 1876 Location record
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) Mail exchange record
        MX,
        /// [RFC 3403](https://tools.ietf.org/html/rfc3403) Naming Authority Pointer
        NAPTR,
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) Name server record
        NS,
        /// [RFC 4034](https://tools.ietf.org/html/rfc4034) Next-Secure record
        NSEC,
        /// [RFC 5155](https://tools.ietf.org/html/rfc5155) NSEC record version 3
        NSEC3,
        /// [RFC 5155](https://tools.ietf.org/html/rfc5155) NSEC3 parameters
        NSEC3PARAM,
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) Null server record, for testing
        NULL,
        /// [RFC 7929](https://tools.ietf.org/html/rfc7929) OpenPGP public key
        OPENPGPKEY,
        /// [RFC 6891](https://tools.ietf.org/html/rfc6891) Option
        OPT,
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) Pointer record
        PTR,
        //  RP,         // 17 RFC 1183 Responsible person
        /// [RFC 4034](https://tools.ietf.org/html/rfc4034) DNSSEC signature: RSASHA256 and RSASHA512, RFC5702
        RRSIG,
        /// [RFC 2535](https://tools.ietf.org/html/rfc2535) (and [RFC 2931](https://tools.ietf.org/html/rfc2931)) Signature, to support [RFC 2137](https://tools.ietf.org/html/rfc2137) Update.
        SIG,
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) and [RFC 2308](https://tools.ietf.org/html/rfc2308) Start of [a zone of] authority record
        SOA,
        /// [RFC 2782](https://tools.ietf.org/html/rfc2782) Service locator
        SRV,
        /// [RFC 4255](https://tools.ietf.org/html/rfc4255) SSH Public Key Fingerprint
        SSHFP,
        /// [RFC draft-ietf-dnsop-svcb-https-03](https://tools.ietf.org/html/draft-ietf-dnsop-svcb-httpssvc-03) DNS SVCB and HTTPS RRs
        SVCB,
        //  TA,         // 32768 N/A DNSSEC Trust Authorities
        //  TKEY,       // 249 RFC 2930 Secret key record
        /// [RFC 6698](https://tools.ietf.org/html/rfc6698) TLSA certificate association
        TLSA,
        /// [RFC 8945](https://tools.ietf.org/html/rfc8945) Transaction Signature
        TSIG,
        /// [RFC 1035](https://tools.ietf.org/html/rfc1035) Text record
        TXT,
        /// Unknown Record type, or unsupported
        Unknown(u16),

        /// This corresponds to a record type of 0, unspecified
        ZERO,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Clone, Encode, Decode, TypeInfo, Eq)]
    #[non_exhaustive]
    pub enum RData {
        A(Ipv4Addr),
        AAAA(Ipv6Addr),
        ANAME(Name),
        CAA(CAA),
        CNAME(Name),
        CSYNC(CSYNC),
        HINFO(HINFO),
        HTTPS(SVCB),
        MX(MX),
        NAPTR(NAPTR),
        NULL(NULL),
        NS(Name),
        OPENPGPKEY(OPENPGPKEY),
        OPT(OPT),
        PTR(Name),
        SOA(SOA),
        SRV(SRV),
        SSHFP(SSHFP),
        SVCB(SVCB),
        TLSA(TLSA),
        TXT(TXT),
        DNSSEC(DNSSECRData),
        Unknown {
            code: u16,
            rdata: NULL,
        },
        #[deprecated(note = "Use None for the RData in the resource record instead")]
        ZERO,
    }

    impl RData {
        pub fn kind(&self) -> RecordType {
            match self {
                RData::A(_) => RecordType::A,
                RData::AAAA(_) => RecordType::AAAA,
                RData::ANAME(_) => RecordType::ANAME,
                RData::CAA(_) => RecordType::CAA,
                RData::CNAME(_) => RecordType::CNAME,
                RData::CSYNC(_) => RecordType::CSYNC,
                RData::HINFO(_) => RecordType::HINFO,
                RData::HTTPS(_) => RecordType::HTTPS,
                RData::MX(_) => RecordType::MX,
                RData::NAPTR(_) => RecordType::NAPTR,
                RData::NULL(_) => RecordType::NULL,
                RData::NS(_) => RecordType::NS,
                RData::OPENPGPKEY(_) => RecordType::OPENPGPKEY,
                RData::OPT(_) => RecordType::OPT,
                RData::PTR(_) => RecordType::PTR,
                RData::SOA(_) => RecordType::SOA,
                RData::SRV(_) => RecordType::SRV,
                RData::SSHFP(_) => RecordType::SSHFP,
                RData::SVCB(_) => RecordType::SVCB,
                RData::TLSA(_) => RecordType::TLSA,
                RData::TXT(_) => RecordType::TXT,
                RData::DNSSEC(_) => RecordType::DNSKEY,
                RData::Unknown { code, rdata } => RecordType::Unknown(*code),
                RData::ZERO => RecordType::ZERO,
            }
        }

        pub fn len(&self) -> u32 {
            todo!()
        }
    }

    impl MaxEncodedLen for RData {
        fn max_encoded_len() -> usize {
            4096
        }
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, Copy, Clone, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Hash)]
    pub struct Ipv4Addr {
        pub octets: [u8; 4],
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, Copy, Clone, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Hash)]
    pub struct Ipv6Addr {
        pub octets: [u8; 16],
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, Hash, Clone, Encode, Decode, TypeInfo, Default, Eq, PartialEq)]
    pub struct Name(pub Vec<u8>);

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct CAA {
        #[doc(hidden)]
        pub issuer_critical: bool,
        #[doc(hidden)]
        pub tag: Property,
        #[doc(hidden)]
        pub value: Value,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub enum Property {
        Issue,
        IssueWild,
        Iodef,
        Unknown(Vec<u8>),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub enum Value {
        Issuer(Option<Name>, Vec<KeyValue>),
        Url(Url),
        Unknown(Vec<u8>),
    }

    impl MaxEncodedLen for Value {
        fn max_encoded_len() -> usize {
            4096
        }
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct KeyValue {
        key: Vec<u8>,
        value: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, Clone, Encode, Decode, Hash, TypeInfo, PartialEq, Eq)]
    pub struct Url {
        serialization: Vec<u8>,
        scheme_end: u32,   // Before ':'
        username_end: u32, // Before ':' (if a password is given) or '@' (if not)
        host_start: u32,
        host_end: u32,
        host: HostInternal,
        port: Option<u16>,
        path_start: u32,             // Before initial '/', if any
        query_start: Option<u32>,    // Before '?', unlike Position::QueryStart
        fragment_start: Option<u32>, // Before '#', unlike Position::FragmentStart
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Copy, Clone, Encode, Decode, Hash, TypeInfo, MaxEncodedLen, Debug, Eq, PartialEq)]
    pub(crate) enum HostInternal {
        None,
        Domain,
        Ipv4(Ipv4Addr),
        Ipv6(Ipv6Addr),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct CSYNC {
        soa_serial: u32,
        immediate: bool,
        soa_minimum: bool,
        type_bit_maps: Vec<RecordType>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct HINFO {
        cpu: Vec<u8>,
        os: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct SVCB {
        svc_priority: u16,
        target_name: Name,
        svc_params: Vec<(SvcParamKey, SvcParamValue)>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Copy)]
    pub enum SvcParamKey {
        /// Mandatory keys in this RR
        Mandatory,
        /// Additional supported protocols
        Alpn,
        /// No support for default protocol
        NoDefaultAlpn,
        /// Port for alternative endpoint
        Port,
        /// IPv4 address hints
        Ipv4Hint,
        /// Encrypted ClientHello info
        EchConfig,
        /// IPv6 address hints
        Ipv6Hint,
        /// Private Use
        Key(u16),
        /// Reserved ("Invalid key")
        Key65535,
        /// Unknown
        Unknown(u16),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub enum SvcParamValue {
        Mandatory(Mandatory),
        Alpn(Alpn),
        NoDefaultAlpn,
        Port(u16),
        Ipv4Hint(IpHint<Ipv4Addr>),
        EchConfig(EchConfig),
        Ipv6Hint(IpHint<Ipv6Addr>),
        Unknown(Unknown),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    #[repr(transparent)]
    pub struct Mandatory(pub Vec<SvcParamKey>);

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    #[repr(transparent)]
    pub struct Alpn(pub Vec<Vec<u8>>);

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    #[repr(transparent)]
    pub struct IpHint<T>(pub Vec<T>);

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    #[repr(transparent)]
    pub struct EchConfig(pub Vec<u8>);

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    #[repr(transparent)]
    pub struct Unknown(pub Vec<u8>);

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct MX {
        preference: u16,
        exchange: Name,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct NAPTR {
        order: u16,
        preference: u16,
        flags: Vec<u8>,
        services: Vec<u8>,
        regexp: Vec<u8>,
        replacement: Name,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Default, Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct NULL {
        pub anything: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct OPENPGPKEY {
        public_key: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Default, Debug, PartialEq, Eq, Clone)]
    pub struct OPT {
        options: hashbrown::HashMap<EdnsCode, EdnsOption>,
    }

    #[repr(transparent)]
    pub struct Options(pub hashbrown::HashMap<EdnsCode, EdnsOption>);

    impl TypeInfo for Options {
        type Identity = hashbrown::HashMap<EdnsCode, EdnsOption>;

        fn type_info() -> scale_info::Type {
            Type::builder()
                .path(::scale_info::Path::new(
                    ::core::stringify!(Options),
                    ::core::module_path!(),
                ))
                .type_params(::scale_info::prelude::vec![])
                .composite(Fields::unnamed().field(|f| f.ty::<[(EdnsCode, EdnsOption)]>()))
        }
    }

    impl ::scale_info::TypeInfo for OPT {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            ::scale_info::Type::builder()
                .path(::scale_info::Path::new(
                    ::core::stringify!(OPT),
                    ::core::module_path!(),
                ))
                .type_params(::scale_info::prelude::vec![])
                .composite(::scale_info::build::Fields::named().field(|f| {
                    f.ty::<Options>()
                        .name(::core::stringify!(options))
                        .type_name("hashbrown::HashMap<EdnsCode, EdnsOption>")
                }))
        }
    }

    impl Encode for OPT {
        fn size_hint(&self) -> usize {
            mem::size_of::<u32>()
                + mem::size_of::<EdnsCode>() * self.options.len()
                + mem::size_of::<EdnsOption>() * self.options.len()
        }
        fn encode_to<W: codec::Output + ?Sized>(&self, dest: &mut W) {
            compact_encode_len_to(dest, self.options.len()).expect("Compact encodes length");
            for i in self.options.iter() {
                i.encode_to(dest);
            }
        }
    }

    pub(crate) fn compact_encode_len_to<W: codec::Output + ?Sized>(
        dest: &mut W,
        len: usize,
    ) -> Result<(), Error> {
        if len > u32::max_value() as usize {
            return Err("Attempted to serialize a collection with too many elements.".into());
        }

        Compact(len as u32).encode_to(dest);
        Ok(())
    }

    impl Decode for OPT {
        fn decode<I: codec::Input>(input: &mut I) -> Result<Self, Error> {
            <Compact<u32>>::decode(input).and_then(move |Compact(len)| {
                input.descend_ref()?;
                let result = Result::from_iter((0..len).map(|_| Decode::decode(input)))
                    .map(|options| OPT { options });
                input.ascend_ref();
                result
            })
        }
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Hash, Debug, Copy, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum EdnsCode {
        /// [RFC 6891, Reserved](https://tools.ietf.org/html/rfc6891)
        Zero,

        /// [RFC 8764l, Apple's Long-Lived Queries, Optional](https://tools.ietf.org/html/rfc8764)
        LLQ,

        /// [UL On-hold](http://files.dns-sd.org/draft-sekar-dns-ul.txt)
        UL,

        /// [RFC 5001, NSID](https://tools.ietf.org/html/rfc5001)
        NSID,
        // 4 Reserved [draft-cheshire-edns0-owner-option] -EXPIRED-
        /// [RFC 6975, DNSSEC Algorithm Understood](https://tools.ietf.org/html/rfc6975)
        DAU,

        /// [RFC 6975, DS Hash Understood](https://tools.ietf.org/html/rfc6975)
        DHU,

        /// [RFC 6975, NSEC3 Hash Understood](https://tools.ietf.org/html/rfc6975)
        N3U,

        /// [RFC 7871, Client Subnet, Optional](https://tools.ietf.org/html/rfc7871)
        Subnet,

        /// [RFC 7314, EDNS EXPIRE, Optional](https://tools.ietf.org/html/rfc7314)
        Expire,

        /// [RFC 7873, DNS Cookies](https://tools.ietf.org/html/rfc7873)
        Cookie,

        /// [RFC 7828, edns-tcp-keepalive](https://tools.ietf.org/html/rfc7828)
        Keepalive,

        /// [RFC 7830, The EDNS(0) Padding](https://tools.ietf.org/html/rfc7830)
        Padding,

        /// [RFC 7901, CHAIN Query Requests in DNS, Optional](https://tools.ietf.org/html/rfc7901)
        Chain,

        /// Unknown, used to deal with unknown or unsupported codes
        Unknown(u16),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Encode, Decode, TypeInfo, Hash)]
    #[non_exhaustive]
    pub enum EdnsOption {
        /// [RFC 6975, DNSSEC Algorithm Understood](https://tools.ietf.org/html/rfc6975)
        DAU(SupportedAlgorithms),

        /// [RFC 6975, DS Hash Understood](https://tools.ietf.org/html/rfc6975)
        DHU(SupportedAlgorithms),

        /// [RFC 6975, NSEC3 Hash Understood](https://tools.ietf.org/html/rfc6975)
        N3U(SupportedAlgorithms),

        /// Unknown, used to deal with unknown or unsupported codes
        Unknown(u16, Vec<u8>),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Encode, Decode, TypeInfo, Copy, Hash)]
    pub struct SupportedAlgorithms {
        // right now the number of Algorithms supported are fewer than 16..
        bit_map: u8,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct SOA {
        mname: Name,
        rname: Name,
        serial: u32,
        refresh: i32,
        retry: i32,
        expire: i32,
        minimum: u32,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct SRV {
        priority: u16,
        weight: u16,
        port: u16,
        target: Name,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct SSHFP {
        algorithm: Algorithm,
        fingerprint_type: FingerprintType,
        fingerprint: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum Algorithm {
        /// Reserved value
        Reserved,

        /// RSA
        RSA,

        /// DSS/DSA
        DSA,

        /// ECDSA
        ECDSA,

        /// Ed25519
        Ed25519,

        /// Ed448
        Ed448,

        /// Unassigned value
        Unassigned(u8),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum FingerprintType {
        /// Reserved value
        Reserved,

        /// SHA-1
        SHA1,

        /// SHA-256
        SHA256,

        /// Unassigned value
        Unassigned(u8),
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct TLSA {
        cert_usage: CertUsage,
        selector: Selector,
        matching: Matching,
        cert_data: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum CertUsage {
        CA,
        Service,
        TrustAnchor,
        DomainIssued,
        Unassigned(u8),
        Private,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum Selector {
        /// Full certificate: the Certificate binary structure as defined in [RFC5280](https://tools.ietf.org/html/rfc5280)
        Full,

        /// SubjectPublicKeyInfo: DER-encoded binary structure as defined in [RFC5280](https://tools.ietf.org/html/rfc5280)
        Spki,

        /// Unassigned at the time of this writing
        Unassigned(u8),

        /// Private usage
        Private,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum Matching {
        /// Exact match on selected content
        Raw,

        /// SHA-256 hash of selected content [RFC6234](https://tools.ietf.org/html/rfc6234)
        Sha256,

        /// SHA-512 hash of selected content [RFC6234](https://tools.ietf.org/html/rfc6234)
        Sha512,

        /// Unassigned at the time of this writing
        Unassigned(u8),

        /// Private usage
        Private,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct TXT {
        txt_data: Vec<Vec<u8>>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Clone, Encode, Decode, TypeInfo, Eq)]
    #[non_exhaustive]
    pub enum DNSSECRData {
        CDNSKEY(DNSKEY),
        CDS(DS),
        DNSKEY(DNSKEY),
        DS(DS),
        KEY(KEY),
        NSEC(NSEC),
        NSEC3(NSEC3),
        NSEC3PARAM(NSEC3PARAM),
        SIG(SIG),
        TSIG(TSIG),
        Unknown { code: u16, rdata: NULL },
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct DNSKEY {
        zone_key: bool,
        secure_entry_point: bool,
        revoke: bool,
        algorithm: Algorithm,
        public_key: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct DS {
        key_tag: u16,
        algorithm: Algorithm,
        digest_type: DigestType,
        digest: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(
        Clone, Encode, Decode, TypeInfo, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug,
    )]
    #[non_exhaustive]
    pub enum DigestType {
        /// [RFC 3658](https://tools.ietf.org/html/rfc3658)
        SHA1,
        /// [RFC 4509](https://tools.ietf.org/html/rfc4509)
        SHA256,
        /// [RFC 5933](https://tools.ietf.org/html/rfc5933)
        GOSTR34_11_94,
        /// [RFC 6605](https://tools.ietf.org/html/rfc6605)
        SHA384,
        /// Undefined
        SHA512,
        /// This is a passthrough digest as ED25519 is self-packaged
        ED25519,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct KEY {
        key_trust: KeyTrust,
        key_usage: KeyUsage,
        signatory: UpdateScope,
        protocol: Protocol,
        algorithm: Algorithm,
        public_key: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum KeyTrust {
        /// Use of the key is prohibited for authentication
        NotAuth,
        /// Use of the key is prohibited for confidentiality
        NotPrivate,
        /// Use of the key for authentication and/or confidentiality is permitted
        AuthOrPrivate,
        /// If both bits are one, the "no key" value, (revocation?)
        DoNotTrust,
    }

    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    pub enum KeyUsage {
        /// key associated with a "user" or "account" at an end entity, usually a host
        Host,
        /// zone key for the zone whose name is the KEY RR owner name
        #[deprecated = "For Zone signing DNSKEY should be used"]
        Zone,
        /// associated with the non-zone "entity" whose name is the RR owner name
        Entity,
        /// Reserved
        Reserved,
    }

    #[deprecated = "Deprecated by RFC3007"]
    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub struct UpdateScope {
        /// this key is authorized to attach,
        ///   detach, and move zones by creating and deleting NS, glue A, and
        ///   zone KEY RR(s)
        pub zone: bool,
        /// this key is authorized to add and
        ///   delete RRs even if there are other RRs with the same owner name
        ///   and class that are authenticated by a SIG signed with a
        ///   different dynamic update KEY
        pub strong: bool,
        /// this key is authorized to add and update RRs for only a single owner name
        pub unique: bool,
        /// The general update signatory field bit has no special meaning, (true if the others are false)
        pub general: bool,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum Protocol {
        /// Not in use
        #[deprecated = "Deprecated by RFC3445"]
        Reserved,
        /// Reserved for use with TLS
        #[deprecated = "Deprecated by RFC3445"]
        TLS,
        /// Reserved for use with email
        #[deprecated = "Deprecated by RFC3445"]
        Email,
        /// Reserved for use with DNSSec (Trust-DNS only supports DNSKEY with DNSSec)
        DNSSec,
        /// Reserved to refer to the Oakley/IPSEC
        #[deprecated = "Deprecated by RFC3445"]
        IPSec,
        /// Undefined
        #[deprecated = "Deprecated by RFC3445"]
        Other(u8),
        /// the key can be used in connection with any protocol
        #[deprecated = "Deprecated by RFC3445"]
        All,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct NSEC {
        next_domain_name: Name,
        type_bit_maps: Vec<RecordType>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct NSEC3 {
        hash_algorithm: Nsec3HashAlgorithm,
        opt_out: bool,
        iterations: u16,
        salt: Vec<u8>,
        next_hashed_owner_name: Vec<u8>,
        type_bit_maps: Vec<RecordType>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo, Copy)]
    pub enum Nsec3HashAlgorithm {
        /// Hash for the Nsec3 records
        SHA1,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct NSEC3PARAM {
        hash_algorithm: Nsec3HashAlgorithm,
        opt_out: bool,
        iterations: u16,
        salt: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct SIG {
        type_covered: RecordType,
        algorithm: Algorithm,
        num_labels: u8,
        original_ttl: u32,
        sig_expiration: u32,
        sig_inception: u32,
        key_tag: u16,
        signer_name: Name,
        sig: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub struct TSIG {
        algorithm: TsigAlgorithm,
        time: u64,
        fudge: u16,
        mac: Vec<u8>,
        oid: u16,
        error: u16,
        other: Vec<u8>,
    }

    #[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
    pub enum TsigAlgorithm {
        /// HMAC-MD5.SIG-ALG.REG.INT (not supported for cryptographic operations)
        HmacMd5,
        /// gss-tsig (not supported for cryptographic operations)
        Gss,
        /// hmac-sha1 (not supported for cryptographic operations)
        HmacSha1,
        /// hmac-sha224 (not supported for cryptographic operations)
        HmacSha224,
        /// hmac-sha256
        HmacSha256,
        /// hmac-sha256-128 (not supported for cryptographic operations)
        HmacSha256_128,
        /// hmac-sha384
        HmacSha384,
        /// hmac-sha384-192 (not supported for cryptographic operations)
        HmacSha384_192,
        /// hmac-sha512
        HmacSha512,
        /// hmac-sha512-256 (not supported for cryptographic operations)
        HmacSha512_256,
        /// Unkown algorithm
        Unknown(Name),
    }
}
