/// A processing subsystem
///
/// Subsystems implement different computer vision tasks, such as AprilTags or object detection.
///
/// A subsystem should be generic, not something that is only used for some specific aspect of a
/// game.
/// For example, note detection for the 2024 game, Crescendo, would go under the object detection
/// subsystem, rather than a brand new subsystem.
///
/// Make sure to pay attention to and respect each subsystem's documentation and structure.
pub(crate) trait Subsystem<'fr>: Sized {
    /// The actual frame processing [Actor]
    ///
    /// May be `Self`
    //type Processor: Actor + Handler<ProcessFrame<Self::Output, Self::Error>>;
    /// The subsystem's configuration type
    type Config;
    type Output: Send + 'static;
    type Error: Debug + Send + 'static;

    /// Initialize the subsystem
    async fn init(cfg: Self::Config) -> Result<Addr<Self>, Self::Error>;

    fn handle(
        &mut self,
        msg: ProcessFrame<Self::Output, Self::Error>,
        ctx: &mut <Self as Actor>::Context,
    ) -> Result<Self::Output, Self::Error>;
}
impl<S: Subsystem> Actor for S {
    type Context = SyncContext<S::Output>;
}
impl<S: Subsystem> Handler<ProcessFrame<S::Output, S::Error>> for S {
    type Result = Result<S::Output, S::Error>;

    fn handle(
        &mut self,
        msg: ProcessFrame<S::Output, S::Error>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        <Self as Subsystem>::handle(self, msg, ctx)
    }
}

/// Actix message for sending a frame to a subsystem for processing
pub(crate) struct ProcessFrame<R, E>
where
    R: Send + 'static,
    E: Debug + Send + 'static,
{
    pub buf: Arc<Vec<u8>>,
    _marker: PhantomData<(R, E)>,
}
impl<R: Send + 'static, E: Debug + Send + 'static> Message for ProcessFrame<R, E> {
    type Result = Result<R, E>;
}
