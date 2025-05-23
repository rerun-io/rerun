(function() {
    var type_impls = Object.fromEntries([["re_view",[]],["re_view_spatial",[]],["re_viewer_context",[]]]);
    if (window.register_type_impls) {
        window.register_type_impls(type_impls);
    } else {
        window.pending_type_impls = type_impls;
    }
})()
//{"start":55,"fragment_lengths":[14,23,25]}