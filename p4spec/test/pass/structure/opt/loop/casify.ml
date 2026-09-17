open Lang
open Util.Source
open Pass__Structure__Ol__Ast
module Casify = Pass__Structure__Opt__Loop__Casify
module Optimize = Pass__Structure__Optimize

let text_exp text = Il.TextE text $$ (no_region, Il.TextT)
let exp_target = Il.VarE ("x" $ no_region) $$ (no_region, Il.TextT)
let guard text = CmpG (`EqOp, `BoolT, text_exp text)
let output text = ReturnI (text_exp text) $ no_region

let branch text reverse text_output =
  let exp_l, exp_r =
    if reverse then (text_exp text, exp_target) else (exp_target, text_exp text)
  in
  let exp = Il.CmpE (`EqOp, `BoolT, exp_l, exp_r) in
  let exp = exp $$ (no_region, Il.BoolT) in
  IfI (exp, [], [ output text_output ]) $ no_region

let case cases total =
  let cases =
    List.map
      (fun (text, texts_output) -> (guard text, List.map output texts_output))
      cases
  in
  CaseI (exp_target, cases, total) $ no_region

let check_block name block_expect block =
  if block <> block_expect then
    failwith
      (Printf.sprintf "%s\nexpected:\n%s\nactual:\n%s" name
         (Pass__Structure__Ol__Print.string_of_block block_expect)
         (Pass__Structure__Ol__Print.string_of_block block))

let test_if_case_preserves_prefix () =
  List.iter
    (fun total ->
      let block =
        [
          branch "b" false "B";
          case [ ("a", [ "A" ]); ("b", [ "C" ]); ("c", [ "D" ]) ] total;
        ]
      in
      let block_expect =
        [ case [ ("a", [ "A" ]); ("b", [ "B"; "C" ]); ("c", [ "D" ]) ] total ]
      in
      let block = Casify.apply Runtime.Dynamic_Sl.Envs.TDEnv.empty block in
      check_block "if-case preserves prefix" block_expect block)
    [ false; true ]

let test_case_if_preserves_prefix () =
  List.iter
    (fun total ->
      let block =
        [
          case [ ("a", [ "A" ]); ("b", [ "B" ]); ("c", [ "D" ]) ] total;
          branch "b" false "C";
        ]
      in
      let block_expect =
        [ case [ ("a", [ "A" ]); ("b", [ "B"; "C" ]); ("c", [ "D" ]) ] false ]
      in
      let block = Casify.apply Runtime.Dynamic_Sl.Envs.TDEnv.empty block in
      check_block "case-if preserves prefix" block_expect block)
    [ false; true ]

let test_case_case_preserves_prefix_and_appended_guards () =
  let block =
    [
      case [ ("a", [ "A" ]); ("b", [ "B" ]) ] false;
      case [ ("c", [ "C" ]); ("b", [ "D" ]); ("c", [ "E" ]) ] false;
    ]
  in
  let block_expect =
    [ case [ ("a", [ "A" ]); ("b", [ "B"; "D" ]); ("c", [ "C"; "E" ]) ] false ]
  in
  let block = Casify.apply Runtime.Dynamic_Sl.Envs.TDEnv.empty block in
  check_block "case-case preserves prefix and appended guards" block_expect
    block

let test_reversed_equality_preserves_reachable_prefix () =
  let block =
    [ branch "a" false "A"; branch "b" false "B"; branch "b" true "C" ]
  in
  let block_expect = [ case [ ("a", [ "A" ]); ("b", [ "B"; "C" ]) ] false ] in
  List.iter
    (fun final ->
      let block =
        Optimize.optimize ~final Runtime.Dynamic_Sl.Envs.TDEnv.empty block
      in
      check_block "optimizer preserves reachable prefix" block_expect block)
    [ false; true ]

let () =
  test_if_case_preserves_prefix ();
  test_case_if_preserves_prefix ();
  test_case_case_preserves_prefix_and_appended_guards ();
  test_reversed_equality_preserves_reachable_prefix ()
