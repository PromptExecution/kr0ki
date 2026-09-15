// Playb00k fixture: SysML v2 structural view of kr0ki's implemented render path.
// Production ModelSnapshot -> UFO -> recognizer -> ViewDefinition remains upstream.
package kr0ki_playbook {
    part def RenderRoute;
    part def FsCache;
    part def RenderBackend;
    part def HttpKrokiBackend :> RenderBackend;
    part def RenderedArtifact;
    part def RenderService {
        part cache : FsCache;
        part backend : RenderBackend;
    }
    part render_flow {
        part route : RenderRoute;
        part service : RenderService;
        part renderer : HttpKrokiBackend;
        part artifact : RenderedArtifact;
    }
}
