let () =
  let channel = open_in_bin Sys.argv.(1) in
  let text = really_input_string channel (in_channel_length channel) in
  close_in channel;
  let marker, replacement, prefix =
    if Array.length Sys.argv = 3 && Sys.argv.(2) = "v1model" then
      ( "module Make (Spec : Spec.S) : Sim.ARCH = struct",
        "module Make (Spec : Spec.S) = struct",
        "open Backend_sim\nopen Backend_sim.V1model" )
    else (") : SIM = struct", ") = struct", "open Backend_sim")
  in
  let positions = ref [] in
  for idx = 0 to String.length text - String.length marker do
    if String.sub text idx (String.length marker) = marker then
      positions := idx :: !positions
  done;
  match !positions with
  | [ idx ] ->
      print_endline prefix;
      print_string (String.sub text 0 idx);
      print_string replacement;
      print_string
        (String.sub text
           (idx + String.length marker)
           (String.length text - idx - String.length marker))
  | _ -> failwith "expected exactly one simulator result signature"
