// PSA replication paths with persistent ingress counter state

#include <core.p4>
#include <bmv2/psa.p4>


typedef bit<48>  EthernetAddress;

header ethernet_t {
    EthernetAddress dstAddr;
    EthernetAddress srcAddr;
    bit<16>         etherType;
}

struct empty_metadata_t {
}

struct metadata_t {
}

struct headers_t {
    ethernet_t       ethernet;
}

parser IngressParserImpl(packet_in pkt,
                         out headers_t hdr,
                         inout metadata_t user_meta,
                         in psa_ingress_parser_input_metadata_t istd,
                         in empty_metadata_t resubmit_meta,
                         in empty_metadata_t recirculate_meta)
{
    state start {
        pkt.extract(hdr.ethernet);
        transition accept;
    }
}

control cIngress(inout headers_t hdr,
                 inout metadata_t user_meta,
                 in    psa_ingress_input_metadata_t  istd,
                 inout psa_ingress_output_metadata_t ostd)
{
    Counter<bit<10>,bit<12>>(1024, PSA_CounterType_t.PACKETS) counter;

    action execute() {
        counter.count(256);
    }

    table tbl {
        actions = { execute; }
        default_action = execute;
    }
    apply {
        send_to_port(ostd, (PortId_t) 1);
        if (hdr.ethernet.etherType == 1) {
            ostd.clone = true;
            ostd.clone_session_id = (CloneSessionId_t) 5;
            ostd.drop = true;
        } else if (hdr.ethernet.etherType == 2) {
            if (istd.packet_path == PSA_PacketPath_t.NORMAL) {
                ostd.clone = true;
                ostd.clone_session_id = (CloneSessionId_t) 5;
                ostd.resubmit = true;
            } else {
                send_to_port(ostd, (PortId_t) 9);
            }
        } else if (hdr.ethernet.etherType == 3) {
            multicast(ostd, (MulticastGroup_t) 7);
        } else if (hdr.ethernet.etherType == 5) {
            if (istd.packet_path == PSA_PacketPath_t.NORMAL) {
                send_to_port(ostd, (PortId_t) 0xfffffffa);
            } else {
                send_to_port(ostd, (PortId_t) 9);
            }
        }
        tbl.apply();
    }
}

parser EgressParserImpl(packet_in buffer,
                        out headers_t hdr,
                        inout metadata_t user_meta,
                        in psa_egress_parser_input_metadata_t istd,
                        in empty_metadata_t normal_meta,
                        in empty_metadata_t clone_i2e_meta,
                        in empty_metadata_t clone_e2e_meta)
{
    state start {
        buffer.extract(hdr.ethernet);
        transition accept;
    }
}

control cEgress(inout headers_t hdr,
                inout metadata_t user_meta,
                in    psa_egress_input_metadata_t  istd,
                inout psa_egress_output_metadata_t ostd)
{
    apply {
        if (hdr.ethernet.etherType == 4 && istd.packet_path == PSA_PacketPath_t.NORMAL_UNICAST) {
            ostd.clone = true;
            ostd.clone_session_id = (CloneSessionId_t) 5;
            ostd.drop = true;
        } else if (hdr.ethernet.etherType == 5 && istd.egress_port == (PortId_t) 0xfffffffa) {
            ostd.clone = true;
            ostd.clone_session_id = (CloneSessionId_t) 5;
        }
    }
}

control CommonDeparserImpl(packet_out packet,
                           inout headers_t hdr)
{
    apply {
        packet.emit(hdr.ethernet);
    }
}

control IngressDeparserImpl(packet_out buffer,
                            out empty_metadata_t clone_i2e_meta,
                            out empty_metadata_t resubmit_meta,
                            out empty_metadata_t normal_meta,
                            inout headers_t hdr,
                            in metadata_t meta,
                            in psa_ingress_output_metadata_t istd)
{
    CommonDeparserImpl() cp;
    apply {
        cp.apply(buffer, hdr);
    }
}

control EgressDeparserImpl(packet_out buffer,
                           out empty_metadata_t clone_e2e_meta,
                           out empty_metadata_t recirculate_meta,
                           inout headers_t hdr,
                           in metadata_t meta,
                           in psa_egress_output_metadata_t istd,
                           in psa_egress_deparser_input_metadata_t edstd)
{
    CommonDeparserImpl() cp;
    apply {
        cp.apply(buffer, hdr);
    }
}

IngressPipeline(IngressParserImpl(),
                cIngress(),
                IngressDeparserImpl()) ip;

EgressPipeline(EgressParserImpl(),
               cEgress(),
               EgressDeparserImpl()) ep;

PSA_Switch(ip, PacketReplicationEngine(), ep, BufferingQueueingEngine()) main;
