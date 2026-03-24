from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from core.prober import TentacleProber
from core.dehydrator import TentacleDehydrator
import logging

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")

app = FastAPI(title="Helix Tentacle", description="渐进式文档探查与脱水器官")

prober = TentacleProber()
dehydrator = TentacleDehydrator()

class DehydrateRequest(BaseModel):
    text: str
    purpose: str = "提取核心摘要"
    model: str | None = None  # 可选，指定使用的模型

class DehydrateResponse(BaseModel):
    outline: str
    dehydrated_content: str

@app.post("/v1/tentacle/process", response_model=DehydrateResponse)
async def process_document(req: DehydrateRequest):
    if not req.text.strip():
        raise HTTPException(status_code=400, detail="Text cannot be empty")
        
    # 1. 探查轮廓
    outline = await prober.probe_outline(req.text, req.model)
    
    # 2. 定向脱水
    dehydrated_content = await dehydrator.dehydrate(req.text, outline, req.purpose, req.model)
    
    return DehydrateResponse(
        outline=outline,
        dehydrated_content=dehydrated_content
    )

if __name__ == "__main__":
    import uvicorn
    # 触手服务默认跑在 8010 端口
    uvicorn.run("main:app", host="0.0.0.0", port=8010, reload=True)
