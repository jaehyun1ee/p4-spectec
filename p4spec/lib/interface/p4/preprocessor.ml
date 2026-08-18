open Core

let preprocess includes path =
  let cmd =
    String.concat ~sep:" "
      ([ "cc" ]
      @ List.map ~f:(Printf.sprintf "-I%s") includes
      @ [ "-undef"; "-nostdinc"; "-E"; "-x"; "c"; path ])
  in
  let in_chan = Core_unix.open_process_in cmd in
  let program = In_channel.input_all in_chan in
  let status = Core_unix.close_process_in in_chan in
  match status with
  | Ok () -> program
  | Error _ -> failwith (Core_unix.Exit_or_signal.to_string_hum status)
