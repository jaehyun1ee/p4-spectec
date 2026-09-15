#include <core.p4>
#include <ebpf_model.p4>

header Headers { bit<8> flag; bit<8> index; }
parser Parse(packet_in pkt, out Headers hdr) {
    CounterArray(2, false) parsed;
    state start {
        pkt.extract(hdr);
        parsed.increment(0);
        transition select(hdr.flag) { 0: reject; default: accept; }
    }
}
control Filter(inout Headers hdr, out bool pass) {
    CounterArray(2, true) counted;
    apply {
        counted.increment((bit<32>) hdr.index);
        counted.add((bit<32>) hdr.index, 2);
        pass = hdr.flag == 1;
    }
}
ebpfFilter(Parse(), Filter()) main;
