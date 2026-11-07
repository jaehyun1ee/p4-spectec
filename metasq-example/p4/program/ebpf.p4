header H { bit<8> src; bit<8> dst; }
extern packet_in { void extract(out H hdr); }
parser parse(packet_in packet, out H hdr);
control filter(in H hdr, out bool pass);
package ebpfFilter(parse prs, filter filt);
