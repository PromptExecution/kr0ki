// Playb00k fixture: KerML view of the implemented Box-5 Rust flow.
// It does not replace the upstream UFO/model pipeline.
package kr0ki_playbook {
    classifier RenderRoute;
    classifier RenderService;
    classifier FsCache;
    classifier HttpKrokiBackend;
    classifier KrokiRenderer;
    classifier RenderedArtifact;
    dependency routeToService : RenderRoute -> RenderService;
    dependency serviceToCache : RenderService -> FsCache;
    dependency serviceToBackend : RenderService -> HttpKrokiBackend;
    dependency backendToRenderer : HttpKrokiBackend -> KrokiRenderer;
    dependency serviceToArtifact : RenderService -> RenderedArtifact;
}
