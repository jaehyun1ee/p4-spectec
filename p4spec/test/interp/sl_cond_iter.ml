open Util.Source
open Lang
module Typ = Runtime.Type.Typ
module Value = Runtime.Value
module Run = Runtime.Dynamic_Runner.Signature

module Extern = struct
  module Cache = struct
    let cache_on () = ()
    let cache_off () = ()
  end

  let eval_extern_rel _ _ : Run.rel_result =
    failwith "unexpected extern relation"

  let eval_extern_func _ _ _ : Run.func_result =
    failwith "unexpected extern function"

  let checkpoint () = 0
  let seff _ _ = false
  let clear () = ()
  let init_mode _ = Ok ()
end

let id name = name $ no_region
let exp_bool cond = Il.BoolE cond $$ (no_region, Typ.Make.bool.it)
let instr instr' = instr' $$ (no_region, { Sl.iid = 0 })

let check det hold =
  let module Interp = Interp_sl.Interp.Make (Interface.P4) (Extern) () in
  let typ = Typ.Make.bool in
  let typ_list = Typ.Make.list typ in
  let typ_lists = Typ.Make.list typ_list in
  let typ_opt = Typ.Make.opt typ_lists in
  let var = (id "b", typ, []) in
  let var_list = (id "b", typ, [ Il.List ]) in
  let var_lists = (id "b", typ, [ Il.List; Il.List ]) in
  let exp = Il.VarE (id "b") $$ (no_region, typ.it) in
  let exp_list =
    Il.IterE (exp, (Il.List, [ var ])) $$ (no_region, typ_list.it)
  in
  let exp_lists =
    Il.IterE (exp_list, (Il.List, [ var_list ])) $$ (no_region, typ_lists.it)
  in
  let exp_opt =
    Il.IterE (exp_lists, (Il.Opt, [ var_lists ])) $$ (no_region, typ_opt.it)
  in
  let iterexps =
    [ (Il.List, [ var ]); (Il.List, [ var_list ]); (Il.Opt, [ var_lists ]) ]
  in
  let block = [ instr (Sl.ReturnI (exp_bool true)) ] in
  let instr_cond =
    if hold then
      Sl.HoldI
        (id "True", Domain.Mixfix.Arg exp, iterexps, Sl.HoldH (block, false))
    else Sl.IfI (exp, iterexps, block, false)
  in
  let rel_signature = (Domain.Mixfix.Arg typ $ no_region, [ 0 ]) in
  let spec =
    [
      Sl.RelD
        ( id "True",
          rel_signature,
          [ exp ],
          [
            instr
              (Sl.IfI
                 (exp, [], [ instr (Sl.ResultI (rel_signature, [])) ], false));
          ],
          None,
          [] )
      $ no_region;
      Sl.FuncDecD
        ( id "entry",
          [],
          [ Sl.ExpP (typ_opt, exp_opt) $ no_region ],
          typ,
          [ instr instr_cond ],
          Some [ instr (Sl.ReturnI (exp_bool false)) ],
          [] )
      $ no_region;
    ]
  in
  (match Interp.init ~cache:false ~det ~guard:true spec with
  | Ok () -> ()
  | Error error -> failwith error.msg);
  List.iter
    (fun (conds_opt, expected) ->
      let value_opt =
        Option.map
          (fun conds ->
            Value.Make.list typ_lists
              (List.map
                 (fun conds ->
                   Value.Make.list typ_list (List.map Value.Make.bool conds))
                 conds))
          conds_opt
      in
      let value = Value.Make.opt typ_opt value_opt in
      match Interp.eval_func "entry" [] [ value ] with
      | Run.Pass value -> assert (Value.Get.bool value = expected)
      | Run.Fail (_, msg) -> failwith msg)
    [
      (Some [ [ true ]; [ true ] ], true);
      (Some [ [ true ]; [ false ] ], false);
      (Some [], true);
      (Some [ [] ], true);
      (None, false);
    ]

let () =
  List.iter (fun det -> List.iter (check det) [ false; true ]) [ false; true ]
