#include<ebpf.p4>
parser prs(packet_in p, out H hdr) {
  state start {
    p.extract(hdr);
    transition accept;
  }
}
control flt(in H hdr, out bool pass) {
  apply {
    if (hdr.dst == 0x42) { pass = false; }
    else { pass = true; }
  }
}
ebpfFilter(prs(), flt()) main;
