use codec::{Decode, Encode};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use trust_dns_proto::rr::RecordType;

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

#[cfg(feature = "std")]
impl Into<codec_type::RecordType> for RecordType {
    fn into(self) -> codec_type::RecordType {
        match self {
            RecordType::A => codec_type::RecordType::A,
            RecordType::AAAA => codec_type::RecordType::AAAA,
            RecordType::ANAME => codec_type::RecordType::ANAME,
            RecordType::ANY => codec_type::RecordType::ANY,
            RecordType::AXFR => codec_type::RecordType::AXFR,
            RecordType::CAA => codec_type::RecordType::CAA,
            RecordType::CDS => codec_type::RecordType::CDS,
            RecordType::CDNSKEY => codec_type::RecordType::CDNSKEY,
            RecordType::CNAME => codec_type::RecordType::CNAME,
            RecordType::CSYNC => codec_type::RecordType::CSYNC,
            RecordType::DNSKEY => codec_type::RecordType::DNSKEY,
            RecordType::DS => codec_type::RecordType::DS,
            RecordType::HINFO => codec_type::RecordType::HINFO,
            RecordType::HTTPS => codec_type::RecordType::HTTPS,
            RecordType::IXFR => codec_type::RecordType::IXFR,
            RecordType::KEY => codec_type::RecordType::KEY,
            RecordType::MX => codec_type::RecordType::MX,
            RecordType::NAPTR => codec_type::RecordType::NAPTR,
            RecordType::NS => codec_type::RecordType::NS,
            RecordType::NSEC => codec_type::RecordType::NSEC,
            RecordType::NSEC3 => codec_type::RecordType::NSEC3,
            RecordType::NSEC3PARAM => codec_type::RecordType::NSEC3PARAM,
            RecordType::NULL => codec_type::RecordType::NULL,
            RecordType::OPENPGPKEY => codec_type::RecordType::OPENPGPKEY,
            RecordType::OPT => codec_type::RecordType::OPT,
            RecordType::PTR => codec_type::RecordType::PTR,
            RecordType::RRSIG => codec_type::RecordType::RRSIG,
            RecordType::SIG => codec_type::RecordType::SIG,
            RecordType::SOA => codec_type::RecordType::SOA,
            RecordType::SRV => codec_type::RecordType::SRV,
            RecordType::SSHFP => codec_type::RecordType::SSHFP,
            RecordType::SVCB => codec_type::RecordType::SVCB,
            RecordType::TLSA => codec_type::RecordType::TLSA,
            RecordType::TSIG => codec_type::RecordType::TSIG,
            RecordType::TXT => codec_type::RecordType::TXT,
            RecordType::Unknown(unknow) => codec_type::RecordType::Unknown(unknow),
            RecordType::ZERO => codec_type::RecordType::ZERO,
            _ => panic!("unsupported record type"),
        }
    }
}

pub mod codec_type {
    use codec::MaxEncodedLen;
    use scale_info::TypeInfo;

    use super::*;

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

    impl RecordType {
        pub fn all() -> [Self; 36] {
            [
                RecordType::A,
                RecordType::AAAA,
                RecordType::ANAME,
                RecordType::ANY,
                RecordType::AXFR,
                RecordType::CAA,
                RecordType::CDS,
                RecordType::CDNSKEY,
                RecordType::CNAME,
                RecordType::CSYNC,
                RecordType::DNSKEY,
                RecordType::DS,
                RecordType::HINFO,
                RecordType::HTTPS,
                RecordType::IXFR,
                RecordType::KEY,
                RecordType::MX,
                RecordType::NAPTR,
                RecordType::NS,
                RecordType::NSEC,
                RecordType::NSEC3,
                RecordType::NSEC3PARAM,
                RecordType::NULL,
                RecordType::OPENPGPKEY,
                RecordType::OPT,
                RecordType::PTR,
                RecordType::RRSIG,
                RecordType::SIG,
                RecordType::SOA,
                RecordType::SRV,
                RecordType::SSHFP,
                RecordType::SVCB,
                RecordType::TLSA,
                RecordType::TSIG,
                RecordType::TXT,
                RecordType::ZERO,
            ]
        }
    }
}
