"""
main.py - DreamCatcher FastAPI Backend
"""

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from api.routes_media import router as media_router
from api.routes_scan import router as scan_router
from api.routes_execute import router as execute_router

app = FastAPI(
    title="DreamCatcher API",
    description="Local AI Photo Triage Cockpit powered by Moondream and FastAPI",
    version="1.0.0"
)

# Enable CORS for local frontend (Vite default port 5173 / Next 3000)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(media_router)
app.include_router(scan_router)
app.include_router(execute_router)


@app.get("/")
def root():
    return {
        "app": "DreamCatcher API",
        "status": "online",
        "vision_models": ["moondream", "gemma3:4b", "gemini-flash-lite"]
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run("main:app", host="127.0.0.1", port=8080, reload=True)
