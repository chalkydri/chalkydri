macro_rules! rerun_log {
    ( $stream:ident , $name:expr , $log:expr ) => {
        #[cfg(feature = "rerun")]
        {
            $stream.log()
        }
    };
}
