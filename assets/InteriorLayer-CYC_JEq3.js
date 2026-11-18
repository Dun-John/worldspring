import{B as e,C as t,D as n,J as r,K as i,Q as a,S as o,U as s,V as c,W as l,X as u,Z as d,c as f,ct as p,d as m,f as h,h as g,i as _,it as v,lt as y,n as b,nt as x,ot as S,q as C,r as w,s as T,st as E,t as D,tt as O,w as k,x as A}from"./CanvasPool-BdPaUYtA.js";import{V as j,W as M,c as ee,d as te,et as N,h as ne,p as re,s as ie,u as ae}from"./tables-D6eXveAK.js";import{t as oe}from"./canvasUtils-DiiLP5gH.js";import{T as se,d as ce,f as le,i as ue,p as de}from"./RenderTargetSystem-vctKAVji.js";import{a as fe,i as pe,n as P,o as me,r as he,s as ge}from"./CanvasRenderer-DjZ80NK1.js";import{t as _e}from"./getPo2TextureFromSource-CwbOPFWE.js";import{r as ve}from"./assets-BuvNzaBZ.js";import{c as ye}from"./state-cmwgIRxE.js";import{d as be,s as xe,v as Se}from"./protocol-BNQBj2YR.js";function Ce(e,t,n,r,i,a,o,s){let c=o-n,l=s-r,u=i-n,d=a-r,f=e-n,p=t-r,m=c*c+l*l,h=c*u+l*d,g=c*f+l*p,_=u*u+d*d,v=u*f+d*p,y=1/(m*_-h*h),b=(_*g-h*v)*y,x=(m*v-h*g)*y;return b>=0&&x>=0&&b+x<1}var we=class e extends b{constructor(...t){let n=t[0]??{};n instanceof Float32Array&&(d(a,`use new MeshGeometry({ positions, uvs, indices }) instead`),n={positions:n,uvs:t[1],indices:t[2]}),n={...e.defaultOptions,...n};let r=n.positions||new Float32Array([0,0,1,0,1,1,0,1]),i=n.uvs;i||=n.positions?new Float32Array(r.length):new Float32Array([0,0,1,0,1,1,0,1]);let o=n.indices||new Uint32Array([0,1,2,0,2,3]),s=n.shrinkBuffersToFit,c=new w({data:r,label:`attribute-mesh-positions`,shrinkToFit:s,usage:_.VERTEX|_.COPY_DST}),l=new w({data:i,label:`attribute-mesh-uvs`,shrinkToFit:s,usage:_.VERTEX|_.COPY_DST}),u=new w({data:o,label:`index-mesh-buffer`,shrinkToFit:s,usage:_.INDEX|_.COPY_DST});super({attributes:{aPosition:{buffer:c,format:`float32x2`,stride:8,offset:0},aUV:{buffer:l,format:`float32x2`,stride:8,offset:0}},indexBuffer:u,topology:n.topology}),this.batchMode=`auto`}get positions(){return this.attributes.aPosition.buffer.data}set positions(e){this.attributes.aPosition.buffer.data=e}get uvs(){return this.attributes.aUV.buffer.data}set uvs(e){this.attributes.aUV.buffer.data=e}get indices(){return this.indexBuffer.data}set indices(e){this.indexBuffer.data=e}};we.defaultOptions={topology:`triangle-list`,shrinkBuffersToFit:!1};var Te=we,Ee=class{constructor(){this.batcherName=`default`,this.packAsQuad=!1,this.indexOffset=0,this.attributeOffset=0,this.roundPixels=0,this._batcher=null,this._batch=null,this._textureMatrixUpdateId=-1,this._uvUpdateId=-1}get blendMode(){return this.renderable.groupBlendMode}get topology(){return this._topology||this.geometry.topology}set topology(e){this._topology=e}reset(){this.renderable=null,this.texture=null,this._batcher=null,this._batch=null,this.geometry=null,this._uvUpdateId=-1,this._textureMatrixUpdateId=-1}setTexture(e){this.texture!==e&&(this.texture=e,this._textureMatrixUpdateId=-1)}get uvs(){let e=this.geometry.getBuffer(`aUV`),t=e.data,n=t,r=this.texture.textureMatrix;return r.isSimple||(n=this._transformedUvs,(this._textureMatrixUpdateId!==r._updateID||this._uvUpdateId!==e._updateID)&&((!n||n.length<t.length)&&(n=this._transformedUvs=new Float32Array(t.length)),this._textureMatrixUpdateId=r._updateID,this._uvUpdateId=e._updateID,r.multiplyUvs(t,n))),n}get positions(){return this.geometry.positions}get indices(){return this.geometry.indices}get color(){return this.renderable.groupColorAlpha}get groupTransform(){return this.renderable.groupTransform}get attributeSize(){return this.geometry.positions.length/2}get indexSize(){return this.geometry.indices.length}},De=class{destroy(){}},Oe=class{constructor(e,t){this.localUniforms=new h({uTransformMatrix:{value:new v,type:`mat3x3<f32>`},uColor:{value:new Float32Array([1,1,1,1]),type:`vec4<f32>`},uRound:{value:0,type:`f32`}}),this.localUniformsBindGroup=new m({0:this.localUniforms}),this.renderer=e,this._adaptor=t,this._adaptor.init()}validateRenderable(e){let t=this._getMeshData(e),n=t.batched,r=e.batched;if(t.batched=r,n!==r)return!0;if(r){let n=e._geometry;if(n.indices.length!==t.indexSize||n.positions.length!==t.vertexSize)return t.indexSize=n.indices.length,t.vertexSize=n.positions.length,!0;let r=this._getBatchableMesh(e);return r.texture.uid!==e._texture.uid&&(r._textureMatrixUpdateId=-1),!r._batcher.checkAndUpdateTexture(r,e._texture)}return!1}addRenderable(e,t){let n=this.renderer.renderPipes.batch,r=this._getMeshData(e);if(e.didViewUpdate&&(r.indexSize=e._geometry.indices?.length,r.vertexSize=e._geometry.positions?.length),r.batched){let r=this._getBatchableMesh(e);r.setTexture(e._texture),r.geometry=e._geometry,n.addToBatch(r,t)}else n.break(t),t.add(e)}updateRenderable(e){if(e.batched){let t=this._getBatchableMesh(e);t.setTexture(e._texture),t.geometry=e._geometry,t._batcher.updateElement(t)}}execute(e){if(!e.isRenderable)return;e.state.blend&&(e.state.blendMode=se(e.groupBlendMode,e.texture._source));let t=this.localUniforms;t.uniforms.uTransformMatrix=e.groupTransform,t.uniforms.uRound=this.renderer._roundPixels|e._roundPixels,t.update(),le(e.groupColorAlpha,t.uniforms.uColor,0),this._adaptor.execute(this,e)}_getMeshData(e){var t,n;return(t=e._gpuData)[n=this.renderer.uid]||(t[n]=new De),e._gpuData[this.renderer.uid].meshData||this._initMeshData(e)}_initMeshData(e){return e._gpuData[this.renderer.uid].meshData={batched:e.batched,indexSize:0,vertexSize:0},e._gpuData[this.renderer.uid].meshData}_getBatchableMesh(e){var t,n;return(t=e._gpuData)[n=this.renderer.uid]||(t[n]=new De),e._gpuData[this.renderer.uid].batchableMesh||this._initBatchableMesh(e)}_initBatchableMesh(e){let t=new Ee;return t.renderable=e,t.setTexture(e._texture),t.transform=e.groupTransform,t.roundPixels=this.renderer._roundPixels|e._roundPixels,e._gpuData[this.renderer.uid].batchableMesh=t,t}destroy(){this.localUniforms=null,this.localUniformsBindGroup=null,this._adaptor.destroy(),this._adaptor=null,this.renderer=null}};Oe.extension={type:[p.WebGLPipes,p.WebGPUPipes],name:`mesh`},y.add(Oe);var ke=class extends t{constructor(...e){let t=e[0];t instanceof b&&(d(a,`Mesh: use new Mesh({ geometry, shader }) instead`),t={geometry:t,shader:e[1]},e[3]&&(d(a,`Mesh: drawMode argument has been removed, use geometry.topology instead`),t.geometry.topology=e[3]));let{geometry:n,shader:r,texture:i,roundPixels:o,state:s,...c}=t;super({label:`Mesh`,...c}),this.renderPipeId=`mesh`,this._shader=null,this.allowChildren=!1,this.shader=r??null,this.texture=i??r?.texture??l.WHITE,this.state=s??T.for2d(),this._geometry=n,this._geometry.on(`update`,this.onViewUpdate,this),this.roundPixels=o??!1}get material(){return d(a,`mesh.material property has been removed, use mesh.shader instead`),this._shader}set shader(e){this._shader!==e&&(this._shader=e,this.onViewUpdate())}get shader(){return this._shader}set geometry(e){this._geometry!==e&&(this._geometry?.off(`update`,this.onViewUpdate,this),e.on(`update`,this.onViewUpdate,this),this._geometry=e,this.onViewUpdate())}get geometry(){return this._geometry}set texture(e){e||=l.EMPTY;let t=this._texture;t!==e&&(t&&t.dynamic&&t.off(`update`,this.onViewUpdate,this),e.dynamic&&e.on(`update`,this.onViewUpdate,this),this.shader&&(this.shader.texture=e),this._texture=e,this.onViewUpdate())}get texture(){return this._texture}get batched(){return this._shader||this.state.data&12?!1:this._geometry instanceof Te?this._geometry.batchMode===`auto`?this._geometry.positions.length/2<=100:this._geometry.batchMode===`batch`:!1}get bounds(){return this._geometry.bounds}updateBounds(){this._bounds=this._geometry.bounds}containsPoint(e){let{x:t,y:n}=e;if(!this.bounds.containsPoint(t,n))return!1;let r=this.geometry.getBuffer(`aPosition`).data,i=this.geometry.topology===`triangle-strip`?3:1;if(this.geometry.getIndex()){let e=this.geometry.getIndex().data,a=e.length;for(let o=0;o+2<a;o+=i){let i=e[o]*2,a=e[o+1]*2,s=e[o+2]*2;if(Ce(t,n,r[i],r[i+1],r[a],r[a+1],r[s],r[s+1]))return!0}}else{let e=r.length/2;for(let a=0;a+2<e;a+=i){let e=a*2,i=(a+1)*2,o=(a+2)*2;if(Ce(t,n,r[e],r[e+1],r[i],r[i+1],r[o],r[o+1]))return!0}}return!1}destroy(e){if(super.destroy(e),typeof e==`boolean`?e:e?.texture){let t=typeof e==`boolean`?e:e?.textureSource;this._texture.destroy(t)}this._geometry?.off(`update`,this.onViewUpdate,this),this._texture=null,this._geometry=null,this._shader=null}},Ae=class extends t{constructor(e,t){let{text:n,resolution:r,style:i,anchor:a,width:o,height:s,roundPixels:c,...l}=e;super({...l}),this.batched=!0,this._resolution=null,this._autoResolution=!0,this._didTextUpdate=!0,this._styleClass=t,this.text=n??``,this.style=i,this.resolution=r??null,this.allowChildren=!1,this._anchor=new E({_onUpdate:()=>{this.onViewUpdate()}}),a&&(this.anchor=a),this.roundPixels=c??!1,o!==void 0&&(this.width=o),s!==void 0&&(this.height=s)}get anchor(){return this._anchor}set anchor(e){typeof e==`number`?this._anchor.set(e):this._anchor.copyFrom(e)}set text(e){e=e.toString(),this._text!==e&&(this._text=e,this.onViewUpdate())}get text(){return this._text}set resolution(e){this._autoResolution=e===null,this._resolution=e,this.onViewUpdate()}get resolution(){return this._resolution}get style(){return this._style}set style(e){e||={},this._style?.off(`update`,this.onViewUpdate,this),this._style=e instanceof this._styleClass?e:new this._styleClass(e),this._style.on(`update`,this.onViewUpdate,this),this.onViewUpdate()}get width(){return Math.abs(this.scale.x)*this.bounds.width}set width(e){this._setWidth(e,this.bounds.width)}get height(){return Math.abs(this.scale.y)*this.bounds.height}set height(e){this._setHeight(e,this.bounds.height)}getSize(e){return e||={},e.width=Math.abs(this.scale.x)*this.bounds.width,e.height=Math.abs(this.scale.y)*this.bounds.height,e}setSize(e,t){typeof e==`object`?(t=e.height??e.width,e=e.width):t??=e,e!==void 0&&this._setWidth(e,this.bounds.width),t!==void 0&&this._setHeight(t,this.bounds.height)}containsPoint(e){let t=this.bounds.width,n=this.bounds.height,r=-t*this.anchor.x,i=0;return e.x>=r&&e.x<=r+t&&(i=-n*this.anchor.y,e.y>=i&&e.y<=i+n)}onViewUpdate(){this.didViewUpdate||(this._didTextUpdate=!0),super.onViewUpdate()}destroy(e=!1){this._style?.off(`update`,this.onViewUpdate,this),super.destroy(e),this.owner=null,this._bounds=null,this._anchor=null,(typeof e==`boolean`?e:e?.style)&&this._style.destroy(e),this._style=null,this._text=null}get styleKey(){return`${this._text}:${this._style.styleKey}:${this._resolution}`}};function je(e,t){let n=e[0]??{};return(typeof n==`string`||e[1])&&(d(a,`use new ${t}({ text: "hi!", style }) instead`),n={text:n,style:e[1]}),n}var F=null,I=null;function Me(e,t){F||(F=A.get().createCanvas(256,128),I=F.getContext(`2d`,{willReadFrequently:!0}),I.globalCompositeOperation=`copy`,I.globalAlpha=1),(F.width<e||F.height<t)&&(F.width=u(e),F.height=u(t))}function Ne(e,t,n){for(let r=0,i=4*n*t;r<t;++r,i+=4)if(e[i+3]!==0)return!1;return!0}function Pe(e,t,n,r,i){let a=4*t;for(let t=r,o=r*a+4*n;t<=i;++t,o+=a)if(e[o+3]!==0)return!1;return!0}function Fe(...e){let t=e[0];t.canvas||(t={canvas:e[0],resolution:e[1]});let{canvas:n}=t,r=Math.min(t.resolution??1,1),i=t.width??n.width,a=t.height??n.height,o=t.output;if(Me(i,a),!I)throw TypeError(`Failed to get canvas 2D context`);I.drawImage(n,0,0,i,a,0,0,i*r,a*r);let s=I.getImageData(0,0,i,a).data,c=0,l=0,u=i-1,d=a-1;for(;l<a&&Ne(s,i,l);)++l;if(l===a)return x.EMPTY;for(;Ne(s,i,d);)--d;for(;Pe(s,i,c,l,d);)++c;for(;Pe(s,i,u,l,d);)--u;return++u,++d,I.globalCompositeOperation=`source-over`,I.strokeRect(c,l,u-c,d-l),I.globalCompositeOperation=`copy`,o??=new x,o.set(c/r,l/r,(u-c)/r,(d-l)/r),o}var Ie=class{constructor(e=0,t=0,n=!1){this.first=null,this.items=Object.create(null),this.last=null,this.max=e,this.resetTtl=n,this.size=0,this.ttl=t}clear(){return this.first=null,this.items=Object.create(null),this.last=null,this.size=0,this}delete(e){if(this.has(e)){let t=this.items[e];delete this.items[e],this.size--,t.prev!==null&&(t.prev.next=t.next),t.next!==null&&(t.next.prev=t.prev),this.first===t&&(this.first=t.next),this.last===t&&(this.last=t.prev)}return this}entries(e=this.keys()){let t=Array(e.length);for(let n=0;n<e.length;n++){let r=e[n];t[n]=[r,this.get(r)]}return t}evict(e=!1){if(e||this.size>0){let e=this.first;delete this.items[e.key],--this.size===0?(this.first=null,this.last=null):(this.first=e.next,this.first.prev=null)}return this}expiresAt(e){let t;return this.has(e)&&(t=this.items[e].expiry),t}get(e){let t=this.items[e];if(t!==void 0){if(this.ttl>0&&t.expiry<=Date.now()){this.delete(e);return}return this.moveToEnd(t),t.value}}has(e){return e in this.items}moveToEnd(e){this.last!==e&&(e.prev!==null&&(e.prev.next=e.next),e.next!==null&&(e.next.prev=e.prev),this.first===e&&(this.first=e.next),e.prev=this.last,e.next=null,this.last!==null&&(this.last.next=e),this.last=e,this.first===null&&(this.first=e))}keys(){let e=Array(this.size),t=this.first,n=0;for(;t!==null;)e[n++]=t.key,t=t.next;return e}setWithEvicted(e,t,n=this.resetTtl){let r=null;if(this.has(e))this.set(e,t,!0,n);else{this.max>0&&this.size===this.max&&(r={...this.first},this.evict(!0));let n=this.items[e]={expiry:this.ttl>0?Date.now()+this.ttl:this.ttl,key:e,prev:this.last,next:null,value:t};++this.size===1?this.first=n:this.last.next=n,this.last=n}return r}set(e,t,n=!1,r=this.resetTtl){let i=this.items[e];return n||i!==void 0?(i.value=t,n===!1&&r&&(i.expiry=this.ttl>0?Date.now()+this.ttl:this.ttl),this.moveToEnd(i)):(this.max>0&&this.size===this.max&&this.evict(!0),i=this.items[e]={expiry:this.ttl>0?Date.now()+this.ttl:this.ttl,key:e,prev:this.last,next:null,value:t},++this.size===1?this.first=i:this.last.next=i,this.last=i),this}values(e=this.keys()){let t=Array(e.length);for(let n=0;n<e.length;n++)t[n]=this.get(e[n]);return t}};function Le(e=1e3,t=0,n=!1){if(isNaN(e)||e<0)throw TypeError(`Invalid max value`);if(isNaN(t)||t<0)throw TypeError(`Invalid ttl value`);if(typeof n!=`boolean`)throw TypeError(`Invalid resetTtl value`);return new Ie(e,t,n)}function Re(e){return!!e.tagStyles&&Object.keys(e.tagStyles).length>0}function ze(e){return e.includes(`<`)}function Be(e,t){return e.clone().assign(t)}function Ve(e,t){let n=[],r=t.tagStyles;if(!Re(t)||!ze(e))return n.push({text:e,style:t}),n;let i=[t],a=[],o=``,s=0;for(;s<e.length;){let t=e[s];if(t===`<`){let c=e.indexOf(`>`,s);if(c===-1){o+=t,s++;continue}let l=e.indexOf(`<`,s+1);if(l!==-1&&l<c){o+=t,s++;continue}let u=e.slice(s+1,c);if(u.startsWith(`/`)){let t=u.slice(1).trim();if(a.length>0&&a[a.length-1]===t){o.length>0&&(n.push({text:o,style:i[i.length-1]}),o=``),i.pop(),a.pop(),s=c+1;continue}o+=e.slice(s,c+1),s=c+1;continue}{let t=u.trim();if(r[t]){o.length>0&&(n.push({text:o,style:i[i.length-1]}),o=``);let e=i[i.length-1],l=Be(e,r[t]);i.push(l),a.push(t),s=c+1;continue}o+=e.slice(s,c+1),s=c+1;continue}}o+=t,s++}return o.length>0&&n.push({text:o,style:i[i.length-1]}),n}var He=new Set([10,13]),Ue=new Set([9,32,8192,8193,8194,8195,8196,8197,8198,8200,8201,8202,8287,12288]),We=new Set([45,8208,8211,8212,173]),Ge=/(\r\n|\r|\n)/,Ke=/(?:\r\n|\r|\n)/;function qe(e){return typeof e==`string`&&He.has(e.charCodeAt(0))}function L(e,t){return typeof e==`string`&&Ue.has(e.charCodeAt(0))}function Je(e){return typeof e==`string`&&We.has(e.charCodeAt(0))}function Ye(e){return e===`normal`||e===`pre-line`}function Xe(e){return e===`normal`}function R(e){if(typeof e!=`string`)return``;let t=e.length-1;for(;t>=0&&L(e[t]);)t--;return t<e.length-1?e.slice(0,t+1):e}function Ze(e){let t=[],n=[];if(typeof e!=`string`)return t;for(let r=0;r<e.length;r++){let i=e[r],a=e[r+1];if(L(i,a)||qe(i)){n.length>0&&(t.push(n.join(``)),n.length=0),i===`\r`&&a===`
`?(t.push(`\r
`),r++):t.push(i);continue}n.push(i),Je(i)&&a&&!L(a)&&!qe(a)&&(t.push(n.join(``)),n.length=0)}return n.length>0&&t.push(n.join(``)),t}function Qe(e,t,n,r){let i=n(e),a=[];for(let n=0;n<i.length;n++){let o=i[n],s=o,c=1;for(;i[n+c];){let a=i[n+c];if(!r(s,a,e,n,t))o+=a,s=a,c++;else break}n+=c-1,a.push(o)}return a}var $e=/\r\n|\r|\n/g;function et(e,t,n,r,i,a,o,s,c){let l=Ve(e,t);if(Xe(t.whiteSpace))for(let e=0;e<l.length;e++){let t=l[e];l[e]={text:t.text.replace($e,` `),style:t.style}}let u=[],d=[];for(let e of l){let t=e.text.split(Ge);for(let n=0;n<t.length;n++){let r=t[n];r===`\r
`||r===`\r`||r===`
`?(u.push(d),d=[]):r.length>0&&d.push({text:r,style:e.style})}}(d.length>0||u.length===0)&&u.push(d);let f=n?tt(u,t,r,a,s,c):u,p=[],m=[],h=[],g=[],_=[],v=0,y=t._fontString,b=o(y);b.fontSize===0&&(b.fontSize=t.fontSize,b.ascent=t.fontSize);let x=``,S=!!t.dropShadow,C=t._stroke?.width||0;for(let e of f){let n=0,a=b.ascent,s=b.descent,c=``;for(let t of e){let e=t.style._fontString,l=o(e);e!==x&&(r.font=e,x=e);let u=i(t.text,t.style.letterSpacing,r);n+=u,a=Math.max(a,l.ascent),s=Math.max(s,l.descent),c+=t.text;let d=t.style._stroke?.width||0;d>C&&(C=d),!S&&t.style.dropShadow&&(S=!0)}e.length===0&&(a=b.ascent,s=b.descent),p.push(n),m.push(a),h.push(s),_.push(c);let l=t.lineHeight||a+s;g.push(l+t.leading),v=Math.max(v,n)}let w=C,T=v+w+(t.dropShadow?t.dropShadow.distance:0),E=0;for(let e=0;e<g.length;e++)E+=g[e];return E=Math.max(E,g[0]+w),{width:T,height:E+(t.dropShadow?t.dropShadow.distance:0),lines:_,lineWidths:p,lineHeight:(t.lineHeight||b.fontSize)+t.leading,maxLineWidth:v,fontProperties:b,runsByLine:f,lineAscents:m,lineDescents:h,lineHeights:g,hasDropShadow:S}}function tt(e,t,n,r,i,a){let{letterSpacing:o,whiteSpace:s,wordWrapWidth:c,breakWords:l}=t,u=Ye(s),d=c+o,f={},p=``,m=(e,t)=>{let i=`${e}|${t.styleKey}`,a=f[i];if(a===void 0){let o=t._fontString;o!==p&&(n.font=o,p=o),a=r(e,t.letterSpacing,n)+t.letterSpacing,f[i]=a}return a},h=[];for(let t of e){let e=nt(t),n=h.length,r=t=>{let n=0,r=t;do{let{token:t,style:i}=e[r];n+=m(t,i),r++}while(r<e.length&&e[r].continuesFromPrevious);return n},o=t=>{let n=[],r=t;do n.push({token:e[r].token,style:e[r].style}),r++;while(r<e.length&&e[r].continuesFromPrevious);return n},s=[],c=0,f=!u,p=null,g=()=>{p&&p.text.length>0&&s.push(p),p=null},_=()=>{if(g(),s.length>0){let e=s[s.length-1];e.text=R(e.text),e.text.length===0&&s.pop()}h.push(s),s=[],c=0,f=!1};for(let t=0;t<e.length;t++){let{token:n,style:v,continuesFromPrevious:y}=e[t],b=m(n,v);if(u){let e=L(n),t=p?.text[p.text.length-1]??s[s.length-1]?.text.slice(-1)??``,r=t?L(t):!1;if(e&&r)continue}let x=!y,S=x?r(t):b;if(S>d&&x){if(c>0&&_(),l){let e=o(t);for(let t=0;t<e.length;t++){let n=e[t].token,r=e[t].style,o=Qe(n,l,a,i);for(let e of o){let t=m(e,r);t+c>d&&_(),!p||p.style!==r?(g(),p={text:e,style:r}):p.text+=e,c+=t}}t+=e.length-1}else{let e=o(t);g(),h.push(e.map(e=>({text:e.token,style:e.style}))),f=!1,t+=e.length-1}}else if(S+c>d&&x){if(L(n)){f=!1;continue}_(),p={text:n,style:v},c=b}else if(y&&!l)!p||p.style!==v?(g(),p={text:n,style:v}):p.text+=n,c+=b;else{let e=L(n);if(c===0&&e&&!f)continue;!p||p.style!==v?(g(),p={text:n,style:v}):p.text+=n,c+=b}}if(g(),s.length>0){let e=s[s.length-1];e.text=R(e.text),e.text.length===0&&s.pop()}(s.length>0||h.length===n)&&h.push(s)}return h}function nt(e){let t=[],n=!1;for(let r of e){let e=Ze(r.text),i=!0;for(let a of e){let e=L(a)||qe(a),o=i&&n&&!e;t.push({token:a,style:r.style,continuesFromPrevious:o}),n=!e,i=!1}}return t}var rt={willReadFrequently:!0};function it(e,t,n,r,i){let a=n[e];return typeof a!=`number`&&(a=i(e,t,r)+t,n[e]=a),a}function at(e,t,n,r,i,a,o){let s=n.getContext(`2d`,rt);s.font=t._fontString;let c=0,l=``,u=[],d=Object.create(null),{letterSpacing:f,whiteSpace:p}=t,m=Ye(p),h=Xe(p),g=!m,_=t.wordWrapWidth+f,v=Ze(e);for(let e=0;e<v.length;e++){let n=v[e];if(qe(n)){if(!h){u.push(R(l)),g=!m,l=``,c=0;continue}n=` `}if(m){let e=L(n),t=L(l[l.length-1]);if(e&&t)continue}let p=it(n,f,d,s,r);if(p>_){if(l!==``&&(u.push(R(l)),l=``,c=0),i(n,t.breakWords)){let e=Qe(n,t.breakWords,o,a);for(let t of e){let e=it(t,f,d,s,r);e+c>_&&(u.push(R(l)),g=!1,l=``,c=0),l+=t,c+=e}}else l.length>0&&(u.push(R(l)),l=``,c=0),u.push(R(n)),g=!1,l=``,c=0}else p+c>_&&(g=!1,u.push(R(l)),l=``,c=0),(l.length>0||!L(n)||g)&&(l+=n,c+=p)}let y=R(l);return y.length>0&&u.push(y),u.join(`
`)}var ot={willReadFrequently:!0},z=class e{static get experimentalLetterSpacingSupported(){let t=e._experimentalLetterSpacingSupported;if(t===void 0){let n=A.get().getCanvasRenderingContext2D().prototype;t=e._experimentalLetterSpacingSupported=`letterSpacing`in n||`textLetterSpacing`in n}return t}constructor(e,t,n,r,i,a,o,s,c,l){this.text=e,this.style=t,this.width=n,this.height=r,this.lines=i,this.lineWidths=a,this.lineHeight=o,this.maxLineWidth=s,this.fontProperties=c,l&&(this.runsByLine=l.runsByLine,this.lineAscents=l.lineAscents,this.lineDescents=l.lineDescents,this.lineHeights=l.lineHeights,this.hasDropShadow=l.hasDropShadow)}static measureText(t=` `,n,r=e._canvas,i=n.wordWrap){let a=`${t}-${n.styleKey}-wordWrap-${i}`;if(e._measurementCache.has(a))return e._measurementCache.get(a);if(Re(n)&&ze(t)){let r=et(t,n,i,e._context,e._measureText,e._measureTextAdvance,e.measureFont,e.canBreakChars,e.wordWrapSplit),o=new e(t,n,r.width,r.height,r.lines,r.lineWidths,r.lineHeight,r.maxLineWidth,r.fontProperties,{runsByLine:r.runsByLine,lineAscents:r.lineAscents,lineDescents:r.lineDescents,lineHeights:r.lineHeights,hasDropShadow:r.hasDropShadow});return e._measurementCache.set(a,o),o}let o=n._fontString,s=e.measureFont(o);s.fontSize===0&&(s.fontSize=n.fontSize,s.ascent=n.fontSize,s.descent=0);let c=e._context;c.font=o;let l=(i?e._wordWrap(t,n,r):t).split(Ke),u=Array(l.length),d=0;for(let t=0;t<l.length;t++){let r=e._measureText(l[t],n.letterSpacing,c);u[t]=r,d=Math.max(d,r)}let f=n._stroke?.width??0,p=n.lineHeight||s.fontSize,m=e._adjustWidthForStyle(d,n),h=Math.max(p,s.fontSize+f)+(l.length-1)*(p+n.leading),g=e._adjustHeightForStyle(h,n),_=new e(t,n,m,g,l,u,p+n.leading,d,s);return e._measurementCache.set(a,_),_}static _adjustWidthForStyle(e,t){let n=e+(t._stroke?.width||0);return t.dropShadow&&(n+=t.dropShadow.distance),n}static _adjustHeightForStyle(e,t){let n=e;return t.dropShadow&&(n+=t.dropShadow.distance),n}static _measureText(t,n,r){let{metricWidth:i,metrics:a,letterSpacingVal:o}=e._measureTextCore(t,n,r),s=-(a.actualBoundingBoxLeft??0),c=(a.actualBoundingBoxRight??0)-s;return a.width>0&&(c+=o),Math.max(i,c)}static _measureTextAdvance(t,n,r){return e._measureTextCore(t,n,r).metricWidth}static _measureTextCore(t,n,r){let i=!1;e.experimentalLetterSpacingSupported&&(e.experimentalLetterSpacing?(r.letterSpacing=`${n}px`,r.textLetterSpacing=`${n}px`,i=!0):(r.letterSpacing=`0px`,r.textLetterSpacing=`0px`));let a=r.measureText(t),o=a.width,s=0;return o>0&&(s=i?-n:(e.graphemeSegmenter(t).length-1)*n,o+=s),{metricWidth:o,metrics:a,letterSpacingVal:s}}static _wordWrap(t,n,r=e._canvas){return at(t,n,r,e._measureTextAdvance,e.canBreakWords,e.canBreakChars,e.wordWrapSplit)}static isBreakingSpace(e,t){return L(e,t)}static canBreakWords(e,t){return t}static canBreakChars(e,t,n,r,i){return!0}static wordWrapSplit(t){return e.graphemeSegmenter(t)}static measureFont(t){if(e._fonts[t])return e._fonts[t];let n=e._context;n.font=t;let r=n.measureText(e.METRICS_STRING+e.BASELINE_SYMBOL),i=r.actualBoundingBoxAscent??0,a=r.actualBoundingBoxDescent??0,o={ascent:i,descent:a,fontSize:i+a};return e._fonts[t]=o,o}static clearMetrics(t=``){t?delete e._fonts[t]:e._fonts={}}static get _canvas(){if(!e.__canvas){let t;try{let n=new OffscreenCanvas(0,0);if(n.getContext(`2d`,ot)?.measureText)return e.__canvas=n,n;t=A.get().createCanvas()}catch{t=A.get().createCanvas()}t.width=t.height=10,e.__canvas=t}return e.__canvas}static get _context(){return e.__context||=e._canvas.getContext(`2d`,ot),e.__context}};z.METRICS_STRING=`|ÉqÅ`,z.BASELINE_SYMBOL=`M`,z.BASELINE_MULTIPLIER=1.4,z.HEIGHT_MULTIPLIER=2,z.graphemeSegmenter=(()=>{if(typeof Intl?.Segmenter==`function`){let e=new Intl.Segmenter;return t=>{let n=e.segment(t),r=[],i=0;for(let e of n)r[i++]=e.segment;return r}}return e=>[...e]})(),z.experimentalLetterSpacing=!1,z._fonts={},z._measurementCache=Le(1e3);var B=z,st=[`serif`,`sans-serif`,`monospace`,`cursive`,`fantasy`,`system-ui`];function ct(e){let t=typeof e.fontSize==`number`?`${e.fontSize}px`:e.fontSize,n=e.fontFamily;Array.isArray(e.fontFamily)||(n=e.fontFamily.split(`,`));for(let e=n.length-1;e>=0;e--){let t=n[e].trim();!/([\"\'])[^\'\"]+\1/.test(t)&&!st.includes(t)&&(t=`"${t}"`),n[e]=t}return`${e.fontStyle} ${e.fontVariant} ${e.fontWeight} ${t} ${n.join(`,`)}`}var lt=1e5;function ut(t,n,r,i=0,a=0,o=0){if(t.texture===l.WHITE&&!t.fill)return c.shared.setValue(t.color).setAlpha(t.alpha??1).toHexa();if(!t.fill){let e=n.createPattern(t.texture.source.resource,`repeat`),r=t.matrix.copyTo(v.shared);return r.scale(t.texture.source.pixelWidth,t.texture.source.pixelHeight),e.setTransform(r),e}if(t.fill instanceof me){let e=t.fill,r=n.createPattern(e.texture.source.resource,`repeat`);return oe.applyPatternTransform(r,e.transform,!1),r}if(t.fill instanceof ge){let e=t.fill,s=e.type===`linear`,l=e.textureSpace===`local`,u=1,d=1;l&&r&&(u=r.width+i,d=r.height+i);let f,p=!1;if(s){let{start:t,end:r}=e;f=n.createLinearGradient(t.x*u+a,t.y*d+o,r.x*u+a,r.y*d+o),p=Math.abs(r.x-t.x)<Math.abs((r.y-t.y)*.1)}else{let{center:t,innerRadius:r,outerCenter:i,outerRadius:s}=e;f=n.createRadialGradient(t.x*u+a,t.y*d+o,r*u,i.x*u+a,i.y*d+o,s*u)}if(p&&l&&r){let t=r.lineHeight/d;for(let n=0;n<r.lines.length;n++){let a=(n*r.lineHeight+i/2)/d;e.colorStops.forEach(e=>{let n=a+e.offset*t;n=Math.max(0,Math.min(1,n)),f.addColorStop(Math.floor(n*lt)/lt,c.shared.setValue(e.color).toHex())})}}else e.colorStops.forEach(e=>{f.addColorStop(e.offset,c.shared.setValue(e.color).toHex())});return f}return e(`FillStyle not recognised`,t),`red`}var dt=new x;function ft(e){let t=0;for(let n=0;n<e.length;n++)e.charCodeAt(n)===32&&t++;return t}var pt=new class{getCanvasAndContext(e){let{text:t,style:n,resolution:r=1}=e,i=n._getFinalPadding(),a=B.measureText(t||` `,n),o=Math.ceil(Math.ceil(Math.max(1,a.width)+i*2)*r),s=Math.ceil(Math.ceil(Math.max(1,a.height)+i*2)*r),c=D.getOptimalCanvasAndContext(o,s);return this._renderTextToCanvas(n,i,r,c,a),{canvasAndContext:c,frame:n.trim?Fe({canvas:c.canvas,width:o,height:s,resolution:1,output:dt}):dt.set(0,0,o,s)}}returnCanvasAndContext(e){D.returnCanvasAndContext(e)}_renderTextToCanvas(e,t,n,r,i){if(i.runsByLine&&i.runsByLine.length>0){this._renderTaggedTextToCanvas(i,e,t,n,r);return}let{canvas:a,context:o}=r,s=ct(e),c=i.lines,l=i.lineHeight,u=i.lineWidths,d=i.maxLineWidth,f=i.fontProperties,p=a.height;if(o.resetTransform(),o.scale(n,n),o.textBaseline=e.textBaseline,e._stroke?.width){let t=e._stroke;o.lineWidth=t.width,o.miterLimit=t.miterLimit,o.lineJoin=t.join,o.lineCap=t.cap}o.font=s;let m,h,g=e.dropShadow?2:1,_=(e._stroke?.width??0)/2,v=(l-f.fontSize)/2;l-f.fontSize<0&&(v=0);for(let a=0;a<g;++a){let s=e.dropShadow&&a===0,g=s?Math.ceil(Math.max(1,p)+t*2):0,y=g*n;if(s)this._setupDropShadow(o,e,n,y);else{let n=e._gradientBounds,r=e._gradientOffset;if(n){let a={width:n.width,height:n.height,lineHeight:n.height,lines:i.lines};this._setFillAndStrokeStyles(o,e,a,t,_,r?.x??0,r?.y??0)}else r?this._setFillAndStrokeStyles(o,e,i,t,_,r.x,r.y):this._setFillAndStrokeStyles(o,e,i,t,_);o.shadowColor=`rgba(0,0,0,0)`}for(let n=0;n<c.length;n++){m=_,h=_+n*l+f.ascent+v,m+=this._getAlignmentOffset(u[n],d,e.align);let i=0;if(e.align===`justify`&&e.wordWrap&&n<c.length-1){let e=ft(c[n]);e>0&&(i=(d-u[n])/e)}e._stroke?.width&&this._drawLetterSpacing(c[n],e,r,m+t,h+t-g,!0,i),e._fill!==void 0&&this._drawLetterSpacing(c[n],e,r,m+t,h+t-g,!1,i)}}}_renderTaggedTextToCanvas(e,t,n,r,i){let{canvas:a,context:o}=i,{runsByLine:s,lineWidths:c,maxLineWidth:l,lineAscents:u,lineHeights:d,hasDropShadow:f}=e,p=a.height;o.resetTransform(),o.scale(r,r),o.textBaseline=t.textBaseline;let m=f?2:1,h=t._stroke?.width??0;for(let e of s)for(let t of e){let e=t.style._stroke?.width??0;e>h&&(h=e)}let g=h/2,_=[];for(let e=0;e<s.length;e++){let t=s[e],n=[];for(let e of t){let t=ct(e.style);o.font=t,n.push({width:B._measureText(e.text,e.style.letterSpacing,o),font:t})}_.push(n)}for(let e=0;e<m;++e){let a=f&&e===0,m=a?Math.ceil(Math.max(1,p)+n*2):0,h=m*r;a||(o.shadowColor=`rgba(0,0,0,0)`);let v=g;for(let e=0;e<s.length;e++){let f=s[e],p=c[e],y=u[e],b=d[e],x=_[e],S=g;S+=this._getAlignmentOffset(p,l,t.align);let C=0;if(t.align===`justify`&&t.wordWrap&&e<s.length-1){let e=0;for(let t of f)e+=ft(t.text);e>0&&(C=(l-p)/e)}let w=v+y,T=S+n;for(let e=0;e<f.length;e++){let t=f[e],{width:s,font:c}=x[e];if(o.font=c,o.textBaseline=t.style.textBaseline,t.style._stroke?.width){let e=t.style._stroke;if(o.lineWidth=e.width,o.miterLimit=e.miterLimit,o.lineJoin=e.join,o.lineCap=e.cap,a){if(t.style.dropShadow)this._setupDropShadow(o,t.style,r,h);else{let e=ft(t.text);T+=s+e*C;continue}}else{let r=B.measureFont(c),i=t.style.lineHeight||r.fontSize;o.strokeStyle=ut(e,o,{width:s,height:i,lineHeight:i,lines:[t.text]},n*2,T-n,v)}this._drawLetterSpacing(t.text,t.style,i,T,w+n-m,!0,C)}let l=ft(t.text);T+=s+l*C}T=S+n;for(let e=0;e<f.length;e++){let t=f[e],{width:s,font:c}=x[e];if(o.font=c,o.textBaseline=t.style.textBaseline,t.style._fill!==void 0){if(a){if(t.style.dropShadow)this._setupDropShadow(o,t.style,r,h);else{let e=ft(t.text);T+=s+e*C;continue}}else{let e=B.measureFont(c),r=t.style.lineHeight||e.fontSize,i={width:s,height:r,lineHeight:r,lines:[t.text]};o.fillStyle=ut(t.style._fill,o,i,n*2,T-n,v)}this._drawLetterSpacing(t.text,t.style,i,T,w+n-m,!1,C)}let l=ft(t.text);T+=s+l*C}v+=b}}}_setFillAndStrokeStyles(e,t,n,r,i,a=0,o=0){if(e.fillStyle=t._fill?ut(t._fill,e,n,r*2,a,o):null,t._stroke?.width){let s=i+r*2;e.strokeStyle=ut(t._stroke,e,n,s,a,o)}}_setupDropShadow(e,t,n,r){e.fillStyle=`black`,e.strokeStyle=`black`;let i=t.dropShadow,a=i.color,o=i.alpha;e.shadowColor=c.shared.setValue(a).setAlpha(o).toRgbaString();let s=i.blur*n,l=i.distance*n;e.shadowBlur=s,e.shadowOffsetX=Math.cos(i.angle)*l,e.shadowOffsetY=Math.sin(i.angle)*l+r}_getAlignmentOffset(e,t,n){return n===`right`?t-e:n===`center`?(t-e)/2:0}_drawLetterSpacing(e,t,n,r,i,a=!1,o=0){let{context:s}=n,c=t.letterSpacing,l=!1;if(B.experimentalLetterSpacingSupported&&(B.experimentalLetterSpacing?(s.letterSpacing=`${c}px`,s.textLetterSpacing=`${c}px`,l=!0):(s.letterSpacing=`0px`,s.textLetterSpacing=`0px`)),(c===0||l)&&o===0){a?s.strokeText(e,r,i):s.fillText(e,r,i);return}if(o!==0&&(c===0||l)){let t=e.split(` `),n=r,c=s.measureText(` `).width;for(let e=0;e<t.length;e++)a?s.strokeText(t[e],n,i):s.fillText(t[e],n,i),n+=s.measureText(t[e]).width+c+o;return}let u=r,d=B.graphemeSegmenter(e),f=s.measureText(e).width,p=0;for(let e=0;e<d.length;++e){let t=d[e];a?s.strokeText(t,u,i):s.fillText(t,u,i);let n=``;for(let t=e+1;t<d.length;++t)n+=d[t];p=s.measureText(n).width,u+=f-p+c,t===` `&&(u+=o),f=p}}},mt=class e extends S{constructor(t={}){super(),this.uid=O(`textStyle`),this._tick=0,this._cachedFontString=null,ht(t),t instanceof e&&(t=t._toObject());let n={...e.defaultTextStyle,...t};for(let e in n){let t=e;this[t]=n[e]}this._tagStyles=t.tagStyles??void 0,this.update(),this._tick=0}get align(){return this._align}set align(e){this._align!==e&&(this._align=e,this.update())}get breakWords(){return this._breakWords}set breakWords(e){this._breakWords!==e&&(this._breakWords=e,this.update())}get dropShadow(){return this._dropShadow}set dropShadow(t){this._dropShadow!==t&&(this._dropShadow=typeof t==`object`&&t?this._createProxy({...e.defaultDropShadow,...t}):t?this._createProxy({...e.defaultDropShadow}):null,this.update())}get fontFamily(){return this._fontFamily}set fontFamily(e){this._fontFamily!==e&&(this._fontFamily=e,this.update())}get fontSize(){return this._fontSize}set fontSize(e){this._fontSize!==e&&(this._fontSize=typeof e==`string`?parseInt(e,10):e,this.update())}get fontStyle(){return this._fontStyle}set fontStyle(e){this._fontStyle!==e&&(this._fontStyle=e.toLowerCase(),this.update())}get fontVariant(){return this._fontVariant}set fontVariant(e){this._fontVariant!==e&&(this._fontVariant=e,this.update())}get fontWeight(){return this._fontWeight}set fontWeight(e){this._fontWeight!==e&&(this._fontWeight=e,this.update())}get leading(){return this._leading}set leading(e){this._leading!==e&&(this._leading=e,this.update())}get letterSpacing(){return this._letterSpacing}set letterSpacing(e){this._letterSpacing!==e&&(this._letterSpacing=e,this.update())}get lineHeight(){return this._lineHeight}set lineHeight(e){this._lineHeight!==e&&(this._lineHeight=e,this.update())}get padding(){return this._padding}set padding(e){this._padding!==e&&(this._padding=e,this.update())}get filters(){return this._filters}set filters(e){this._filters!==e&&(this._filters=Object.freeze(e),this.update())}get trim(){return this._trim}set trim(e){this._trim!==e&&(this._trim=e,this.update())}get textBaseline(){return this._textBaseline}set textBaseline(e){this._textBaseline!==e&&(this._textBaseline=e,this.update())}get whiteSpace(){return this._whiteSpace}set whiteSpace(e){this._whiteSpace!==e&&(this._whiteSpace=e,this.update())}get wordWrap(){return this._wordWrap}set wordWrap(e){this._wordWrap!==e&&(this._wordWrap=e,this.update())}get wordWrapWidth(){return this._wordWrapWidth}set wordWrapWidth(e){this._wordWrapWidth!==e&&(this._wordWrapWidth=e,this.update())}get fill(){return this._originalFill}set fill(e){e!==this._originalFill&&(this._originalFill=e,this._isFillStyle(e)&&(this._originalFill=this._createProxy({...he.defaultFillStyle,...e},()=>{this._fill=pe({...this._originalFill},he.defaultFillStyle)})),this._fill=pe(e===0?`black`:e,he.defaultFillStyle),this.update())}get stroke(){return this._originalStroke}set stroke(e){e!==this._originalStroke&&(this._originalStroke=e,this._isFillStyle(e)&&(this._originalStroke=this._createProxy({...he.defaultStrokeStyle,...e},()=>{this._stroke=fe({...this._originalStroke},he.defaultStrokeStyle)})),this._stroke=fe(e,he.defaultStrokeStyle),this.update())}get tagStyles(){return this._tagStyles}set tagStyles(e){this._tagStyles!==e&&(this._tagStyles=e??void 0,this.update())}update(){this._tick++,this._cachedFontString=null,this.emit(`update`,this)}reset(){let t=e.defaultTextStyle;for(let e in t)this[e]=t[e]}assign(e){for(let t in e){let n=t;this[n]=e[t]}return this}get styleKey(){return`${this.uid}-${this._tick}`}get _fontString(){return this._cachedFontString===null&&(this._cachedFontString=ct(this)),this._cachedFontString}_toObject(){return{align:this.align,breakWords:this.breakWords,dropShadow:this._dropShadow?{...this._dropShadow}:null,fill:this._fill?{...this._fill}:void 0,fontFamily:this.fontFamily,fontSize:this.fontSize,fontStyle:this.fontStyle,fontVariant:this.fontVariant,fontWeight:this.fontWeight,leading:this.leading,letterSpacing:this.letterSpacing,lineHeight:this.lineHeight,padding:this.padding,stroke:this._stroke?{...this._stroke}:void 0,textBaseline:this.textBaseline,trim:this.trim,whiteSpace:this.whiteSpace,wordWrap:this.wordWrap,wordWrapWidth:this.wordWrapWidth,filters:this._filters?[...this._filters]:void 0,tagStyles:this._tagStyles?{...this._tagStyles}:void 0}}clone(){return new e(this._toObject())}_getFinalPadding(){let e=0;if(this._filters)for(let t=0;t<this._filters.length;t++)e+=this._filters[t].padding;return Math.max(this._padding,e)}destroy(e=!1){if(this.removeAllListeners(),typeof e==`boolean`?e:e?.texture){let t=typeof e==`boolean`?e:e?.textureSource;this._fill?.texture&&this._fill.texture.destroy(t),this._originalFill?.texture&&this._originalFill.texture.destroy(t),this._stroke?.texture&&this._stroke.texture.destroy(t),this._originalStroke?.texture&&this._originalStroke.texture.destroy(t)}this._fill=null,this._stroke=null,this.dropShadow=null,this._originalStroke=null,this._originalFill=null}_createProxy(e,t){return new Proxy(e,{set:(e,n,r)=>e[n]===r||(e[n]=r,t?.(n,r),this.update(),!0)})}_isFillStyle(e){return(e??null)!==null&&!(c.isColorLike(e)||e instanceof ge||e instanceof me)}};mt.defaultDropShadow={alpha:1,angle:Math.PI/6,blur:0,color:`black`,distance:5},mt.defaultTextStyle={align:`left`,breakWords:!1,dropShadow:null,fill:`black`,fontFamily:`Arial`,fontSize:26,fontStyle:`normal`,fontVariant:`normal`,fontWeight:`normal`,leading:0,letterSpacing:0,lineHeight:0,padding:0,stroke:null,textBaseline:`alphabetic`,trim:!1,whiteSpace:`pre`,wordWrap:!1,wordWrapWidth:100};var V=mt;function ht(t){let n=t;if(typeof n.dropShadow==`boolean`&&n.dropShadow){let e=V.defaultDropShadow;t.dropShadow={alpha:n.dropShadowAlpha??e.alpha,angle:n.dropShadowAngle??e.angle,blur:n.dropShadowBlur??e.blur,color:n.dropShadowColor??e.color,distance:n.dropShadowDistance??e.distance}}if(n.strokeThickness!==void 0){d(a,`strokeThickness is now a part of stroke`);let e=n.stroke,r={};if(c.isColorLike(e))r.color=e;else if(e instanceof ge||e instanceof me)r.fill=e;else if(Object.hasOwnProperty.call(e,`color`)||Object.hasOwnProperty.call(e,`fill`))r=e;else throw Error(`Invalid stroke value.`);t.stroke={...r,width:n.strokeThickness}}if(Array.isArray(n.fillGradientStops)){if(d(a,"gradient fill is now a fill pattern: `new FillGradient(...)`"),!Array.isArray(n.fill)||n.fill.length===0)throw Error(`Invalid fill value. Expected an array of colors for gradient fill.`);n.fill.length!==n.fillGradientStops.length&&e(`The number of fill colors must match the number of fill gradient stops.`);let r=new ge({start:{x:0,y:0},end:{x:0,y:1},textureSpace:`local`}),i=n.fillGradientStops.slice(),o=n.fill.map(e=>c.shared.setValue(e).toNumber());i.forEach((e,t)=>{r.addColorStop(e,o[t])}),t.fill={fill:r}}}var gt=!1;function _t(t){if(gt)return;let n=new r({scaleMode:t.scaleMode});t._resourceId!==n._resourceId&&(gt=!0,e(`Text textureStyle: only scaleMode is applied to a text texture, the other fields are ignored`))}function vt(e,t){let{texture:n,bounds:r}=e,i=t._style._getFinalPadding();s(r,t._anchor,n);let a=t._anchor._x*i*2,o=t._anchor._y*i*2;r.minX-=i-a,r.minY-=i-o,r.maxX-=i-a,r.maxY-=i-o}var yt=class extends ce{},bt=class{constructor(e){this._renderer=e,e.runners.resolutionChange.add(this),e.runners.contextChange.add(this),this._managedTexts=new de({renderer:e,type:`renderable`,onUnload:this.onTextUnload.bind(this),name:`canvasText`})}resolutionChange(){for(let e in this._managedTexts.items){let t=this._managedTexts.items[e];t?._autoResolution&&t.onViewUpdate()}}contextChange(){for(let e in this._managedTexts.items)this._managedTexts.items[e]?.unload()}validateRenderable(e){let t=this._getGpuText(e),n=e.styleKey;return t.currentKey!==n||e._didTextUpdate}addRenderable(e,t){let n=this._getGpuText(e);if(e._didTextUpdate||n.currentKey!==e.styleKey){let t=e._autoResolution?this._renderer.resolution:e.resolution;(n.currentKey!==e.styleKey||e._resolution!==t)&&this._updateGpuText(e),e._didTextUpdate=!1,vt(n,e)}this._renderer.renderPipes.batch.addToBatch(n,t)}updateRenderable(e){let t=this._getGpuText(e);t._batcher.updateElement(t)}_updateGpuText(e){let t=this._getGpuText(e);t.texture&&this._renderer.canvasText.decreaseReferenceCount(t.currentKey),e._resolution=e._autoResolution?this._renderer.resolution:e.resolution,t.texture=this._renderer.canvasText.getManagedTexture(e),t.currentKey=e.styleKey}_getGpuText(e){return e._gpuData[this._renderer.uid]||this.initGpuText(e)}initGpuText(e){let t=new yt;return t.currentKey=`--`,t.renderable=e,t.transform=e.groupTransform,t.bounds={minX:0,maxX:1,minY:0,maxY:0},t.roundPixels=this._renderer._roundPixels|e._roundPixels,e._gpuData[this._renderer.uid]=t,this._managedTexts.add(e),t}onTextUnload(e){let t=e._gpuData[this._renderer.uid];if(!t)return;let{canvasText:n}=this._renderer;n.getReferenceCount(t.currentKey)>0?n.decreaseReferenceCount(t.currentKey):t.texture&&n.returnTexture(t.texture)}destroy(){this._managedTexts.destroy(),this._renderer=null}};bt.extension={type:[p.WebGLPipes,p.WebGPUPipes,p.CanvasPipes],name:`text`};var xt=class{constructor(e,t){this._activeTextures={},this._renderer=e,this._retainCanvasContext=t}getTexture(e,t,n,r){typeof e==`string`&&(d(`8.0.0`,`CanvasTextSystem.getTexture: Use object TextOptions instead of separate arguments`),e={text:e,style:n,resolution:t}),e.style instanceof V||(e.style=new V(e.style)),typeof e.text!=`string`&&(e.text=e.text.toString());let{text:i,style:a,textureStyle:o,autoGenerateMipmaps:s}=e,c=e.resolution??this._renderer.resolution,{frame:l,canvasAndContext:u}=pt.getCanvasAndContext({text:i,style:a,resolution:c}),f=_e(u.canvas,l.width,l.height,c,s,o?.scaleMode);if(a.trim&&(l.pad(a.padding),f.frame.copyFrom(l),f.frame.scale(1/c),f.updateUvs()),a.filters){let e=this._applyFilters(f,a.filters);return this.returnTexture(f),pt.returnCanvasAndContext(u),e}return this._renderer.texture.initSource(f._source),this._retainCanvasContext||pt.returnCanvasAndContext(u),f}returnTexture(e){let t=e.source,r=t.resource;if(this._retainCanvasContext&&r?.getContext){let e=r.getContext(`2d`);e&&pt.returnCanvasAndContext({canvas:r,context:e})}t.resource=null,t.uploadMethodId=`unknown`,t.alphaMode=`no-premultiply-alpha`,n.returnTexture(e)}renderTextToCanvas(){d(`8.10.0`,`CanvasTextSystem.renderTextToCanvas: no longer supported, use CanvasTextSystem.getTexture instead`)}getManagedTexture(e){e._resolution=e._autoResolution?this._renderer.resolution:e.resolution;let t=e.styleKey;if(this._activeTextures[t])return this._increaseReferenceCount(t),this._activeTextures[t].texture;let n=this.getTexture({text:e.text,style:e.style,resolution:e._resolution,textureStyle:e.textureStyle,autoGenerateMipmaps:e.autoGenerateMipmaps});return this._activeTextures[t]={texture:n,usageCount:1},n}decreaseReferenceCount(e){let t=this._activeTextures[e];t&&(t.usageCount--,t.usageCount===0&&(this.returnTexture(t.texture),this._activeTextures[e]=null))}getReferenceCount(e){return this._activeTextures[e]?.usageCount??0}_increaseReferenceCount(e){this._activeTextures[e].usageCount++}_applyFilters(e,t){let n=this._renderer.renderTarget.renderTarget,r=this._renderer.filter.generateFilteredTexture({texture:e,filters:t});return this._renderer.renderTarget.bind({target:n,clear:!1}),r}destroy(){this._renderer=null;for(let e in this._activeTextures)this._activeTextures[e]&&this.returnTexture(this._activeTextures[e].texture);this._activeTextures=null}},St=class extends xt{constructor(e){super(e,!0)}};St.extension={type:[p.CanvasSystem],name:`canvasText`};var Ct=class extends xt{constructor(e){super(e,!1)}};Ct.extension={type:[p.WebGLSystem,p.WebGPUSystem],name:`canvasText`},y.add(St),y.add(Ct),y.add(bt);var wt=class extends Ae{constructor(...e){let t=je(e,`Text`);super(t,V),this.renderPipeId=`text`,t.textureStyle&&(this.textureStyle=t.textureStyle instanceof r?t.textureStyle:new r(t.textureStyle),_t(this.textureStyle)),this.autoGenerateMipmaps=t.autoGenerateMipmaps??C.defaultOptions.autoGenerateMipmaps}updateBounds(){let e=this._bounds,t=this._anchor,n=0,r=0;if(this._style.trim){let{frame:e,canvasAndContext:t}=pt.getCanvasAndContext({text:this.text,style:this._style,resolution:1});pt.returnCanvasAndContext(t),n=e.width,r=e.height}else{let e=B.measureText(this._text,this._style);n=e.width,r=e.height}e.minX=-t._x*n,e.maxX=e.minX+n,e.minY=-t._y*r,e.maxY=e.minY+r}},Tt=new WeakMap;function Et(e,t,n){let r=Tt.get(e);if(r!==void 0)return r;if(r=null,e.floor&&e.floor.length===t*n){let i=new Float32Array(t*n);for(let t=0;t<i.length;t++)i[t]=(e.floor.charCodeAt(t)-48)/2;let a=new Uint8Array(t*n);for(let t of e.ramps??[])a[t]=1;r={nx:t,ny:n,ft:i,ramp:a}}return Tt.set(e,r),r}function Dt(e,t,n){let r=e.ft[n]-e.ft[t];return Math.abs(r)>3&&!e.ramp[t]&&!e.ramp[n]?r:0}var Ot=(e,t,n)=>e.cells[t]>=0&&e.cells[n]>=0&&(e.natural||e.cells[t]===e.cells[n]);function kt(e,t,n,r){let{nx:i,ny:a}=e,o=r*i+n,s=0;for(let[c,l]of[[-1,0],[1,0],[0,-1],[0,1]]){let[u,d]=[n+c,r+l];u<0||d<0||u>=i||d>=a||!Ot(t,o,d*i+u)||(s=Math.max(s,-Dt(e,o,d*i+u)))}return s}function At(e,t,n){let r=t.cells[n];if(r<0)return 0;let i=t.rooms[r]?.raise_ft??0;return!e.ramp[n]&&e.ft[n]-i<=3?i:e.ft[n]}function jt(e,t){let{nx:n,ny:r}=e,i=new Float32Array((n+1)*r),a=new Float32Array(n*(r+1));for(let o=0;o<r;o++)for(let r=0;r<n;r++){let s=o*n+r;r>0&&Ot(t,s-1,s)&&!e.ramp[s-1]&&!e.ramp[s]&&(i[o*(n+1)+r]=e.ft[s]-e.ft[s-1]),o>0&&Ot(t,s-n,s)&&!e.ramp[s-n]&&!e.ramp[s]&&(a[o*n+r]=e.ft[s]-e.ft[s-n])}return{v:i,h:a}}function Mt(e,t,n,r,i){let a=Et(e,t,n),o=i*t+r;return a?a.ft[o]:e.rooms[e.cells[o]]?.raise_ft??0}function Nt(e,t,n,r,i){let a=Et(e,t,n),o=a?kt(a,e,r,i):0;return o<5?null:{cover:`${o>=10?`three-quarters`:`half`} cover from below`,note:`ledge, ${o} ft drop`}}var Pt=e=>!!e&&e.startsWith(`hidden`),Ft=new Set([`water_deep`,`chasm`]),It=new Set([`rope_bridge`,`stone_bridge`,`stepping_stone`]);function Lt(e,t,n){let r=e.levels[t],{nx:i,ny:a}=e,o=new Uint8Array(i*a);for(let e=0;e<i*a;e++){let t=r.cells[e]<0?null:r.rooms[r.cells[e]].kind;t===null||t===`roof`?o[e]=11:t===`open to below`&&(o[e]=2)}let s=new Uint8Array(i*a);for(let e of r.furniture){let t=e.blocks_move&&Ft.has(e.kind),n=0;if(e.blocks_move&&!t&&(n|=2),t)for(let t=e.y;t<e.y+e.h&&t<a;t++)for(let n=e.x;n<e.x+e.w&&n<i;n++)s[t*i+n]=1;e.blocks_move&&e.height_ft>=6&&(n|=1),!e.blocks_move&&(e.height_ft>=1||e.hazard&&!Pt(e.hazard))&&(n|=4);for(let t=e.y;t<e.y+e.h&&t<a;t++)for(let r=e.x;r<e.x+e.w&&r<i;r++)o[t*i+r]|=n}for(let e of r.furniture)if(It.has(e.kind))for(let t=e.y;t<e.y+e.h&&t<a;t++)for(let n=e.x;n<e.x+e.w&&n<i;n++)s[t*i+n]=0;for(let e=0;e<s.length;e++)s[e]&&(o[e]|=2);let c=Et(r,i,a),l=c?new Float32Array(i*a):null;if(c&&l)for(let e=0;e<i*a;e++){if(r.cells[e]<0)continue;l[e]=At(c,r,e);let[t,n]=[e%i,Math.floor(e/i)],s=t=>r.cells[t]>=0&&(r.natural||r.cells[t]===r.cells[e])&&c.ft[e]-c.ft[t]>0&&c.ft[e]-c.ft[t]<=3;(c.ramp[e]||t>0&&s(e-1)||t+1<i&&s(e+1)||n>0&&s(e-i)||n+1<a&&s(e+i))&&(o[e]|=4)}let u=(e,t)=>l&&e>=0&&t>=0&&e<i&&t<a?l[t*i+e]:NaN;if(r.has_stairs&&!/^[uwk]:/.test(e.id)){let[t,n,r,s]=e.stairs;for(let e=n;e<n+s;e++)for(let n=t;n<t+r;n++)n<i&&e<a&&(o[e*i+n]|=4)}let d=new Uint8Array((i+1)*a),f=new Uint8Array(i*(a+1)),p=(e,t,n=1)=>{if(e[0]===t[0]){let r=Math.round(e[0]);for(let o=Math.round(Math.min(e[1],t[1]));o<Math.round(Math.max(e[1],t[1]));o++)o>=0&&o<a&&r>=0&&r<=i&&(d[o*(i+1)+r]||=n)}else{let r=Math.round(e[1]);for(let o=Math.round(Math.min(e[0],t[0]));o<Math.round(Math.max(e[0],t[0]));o++)o>=0&&o<i&&r>=0&&r<=a&&(f[r*i+o]||=n)}};for(let e of r.walls)!e.rail&&!e.arcade&&p(e.a,e.b);r.doors.forEach((e,t)=>e.kind!==`arch`&&n(t)&&p(e.a,e.b,e.kind===`portcullis`?2:1));let[m,h]=[i+4,a+4],g=new Uint8Array(m*h),_=(e,t)=>e>=0&&t>=0&&e<i&&t<a&&r.cells[t*i+e]>=0,v=(e,t)=>{let n=e[0]===t[0],[r,o,s]=n?[Math.round(e[0]),Math.round(Math.min(e[1],t[1])),Math.round(Math.max(e[1],t[1]))]:[Math.round(e[1]),Math.round(Math.min(e[0],t[0])),Math.round(Math.max(e[0],t[0]))];for(let e=o;e<s;e++){let[t,o,s,c]=n?[r-1,e,r,e]:[e,r-1,e,r],l=!_(t,o),[u,d]=l?[t,o]:_(s,c)?[NaN,NaN]:[s,c];if(Number.isNaN(u))continue;let[f,p]=n?[l?-1:1,0]:[0,l?-1:1];for(let e=0;e<2;e++){let[t,n]=[u+f*e,d+p*e];t>=-2&&n>=-2&&t<i+2&&n<a+2&&!_(t,n)&&(g[(n+2)*m+t+2]=1)}}};for(let e of r.doors)e.kind===`arch`&&v(e.a,e.b);for(let e of r.walls)e.arcade&&v(e.a,e.b);let y=g.some(e=>e===1),b=(e,t)=>e>=-2&&t>=-2&&e<i+2&&t<a+2&&g[(t+2)*m+e+2]?0:e<0||t<0||e>=i||t>=a?11:o[t*i+e];return{flags:b,elev:u,fill:(e,t,n,r,i,a)=>{a.fill(NaN);for(let a=0;a<r;a++)for(let r=0;r<n;r++)i[a*n+r]=b(e+r,t+a);if(l)for(let i=0;i<r;i++)for(let r=0;r<n;r++)a[i*n+r]=u(e+r,t+i)},wallW:(e,t)=>e>=0&&e<=i&&t>=0&&t<a&&d[t*(i+1)+e]===1,wallN:(e,t)=>e>=0&&e<i&&t>=0&&t<=a&&f[t*i+e]===1,barW:(e,t)=>e>=0&&e<=i&&t>=0&&t<a&&d[t*(i+1)+e]===2,barN:(e,t)=>e>=0&&e<i&&t>=0&&t<=a&&f[t*i+e]===2,bounds:y?[-2,-2,i+2,a+2]:[0,0,i,a]}}var Rt=(e,t,n)=>{let[r,i]=[t-e.x0,n-e.y0];return r>=0&&i>=0&&r<e.w&&i<e.h&&e.bits[i*e.w+r]>0};function zt(e,t,n,r,i){let a={x0:t,y0:n,w:r,h:i,flags:new Uint8Array(r*i),elev:new Float32Array(r*i),wv:new Uint8Array((r+1)*i),wh:new Uint8Array(r*(i+1))};if(e.fill)e.fill(t,n,r,i,a.flags,a.elev);else for(let o=0;o<i;o++)for(let i=0;i<r;i++)a.flags[o*r+i]=e.flags(t+i,n+o),a.elev[o*r+i]=e.elev(t+i,n+o);if(e.noWalls)return a;for(let o=0;o<i;o++)for(let i=0;i<=r;i++)a.wv[o*(r+1)+i]=+!!e.wallW(t+i,n+o);for(let o=0;o<=i;o++)for(let i=0;i<r;i++)a.wh[o*r+i]=+!!e.wallN(t+i,n+o);return a}function Bt(e,t,n){let{w:r,h:i,flags:a,elev:o,wv:s,wh:c}=e,l=t.x-e.x0,u=t.y-e.y0,d=t.range,f=Math.floor(l),p=Math.floor(u);if(f<0||p<0||f>=r||p>=i)return;n[p*r+f]=1;let m=o[p*r+f],h=Number.isNaN(m)?NaN:m+t.above,g=Math.max(240,Math.ceil(2*Math.PI*d*1.6));for(let e=0;e<g;e++){let t=(e+.5)/g*Math.PI*2,m=Math.cos(t),_=Math.sin(t),v=f,y=p,b=m>0?1:-1,x=_>0?1:-1,S=Math.abs(1/m),C=Math.abs(1/_),w=Math.abs(m)<1e-12?1/0:(m>0?v+1-l:l-v)*S,T=Math.abs(_)<1e-12?1/0:(_>0?y+1-u:u-y)*C,E=-1/0;for(;;){let e;if(w<T){if(e=w,s[y*(r+1)+(b>0?v+1:v)])break;v+=b,w+=S}else{if(e=T,c[(x>0?y+1:y)*r+v])break;y+=x,T+=C}if(e>d||v<0||y<0||v>=r||y>=i)break;let t=y*r+v,l=a[t];if(l&16)break;let u=!0;if(!Number.isNaN(h)){let n=o[t];if(!Number.isNaN(n)){let t=(n-h)/(e+.5);u=t>=E-.04,t>E&&(E=t)}}if(u&&(n[t]=1),l&9)break}}}function Vt(e,t,n,r){let i=(e,t)=>e>=r.x0&&t>=r.y0&&e<r.x0+r.w&&t<r.y0+r.h?(t-r.y0)*r.w+(e-r.x0):-1;for(let a of e.furniture){if(a.kind!==`niche`||!r.bits[i(a.x,a.y)])continue;let o=a.back===void 0?[[0,-1],[-1,0],[1,0],[0,1]]:[[[0,-1],[1,0],[0,1],[-1,0]][a.back]];for(let[s,c]of o){let[o,l]=[a.x+s,a.y+c];if(o>=0&&l>=0&&o<t&&l<n&&e.cells[l*t+o]>=0)continue;let u=i(o,l);u>=0&&(r.bits[u]=1);break}}}var Ht=2048;function Ut(e,t,n,r,i){let a=i??(t.length?[t[0].x,t[0].y]:[0,0]),o=e=>Math.max(Math.abs(e.x-a[0]),Math.abs(e.y-a[1]));(e=>{let[t,n,r,i]=[1/0,1/0,-1/0,-1/0];for(let a of e)[t,n,r,i]=[Math.min(t,a.x-a.range),Math.min(n,a.y-a.range),Math.max(r,a.x+a.range),Math.max(i,a.y+a.range)];return(r-t)*(i-n)>Ht*Ht||!Number.isFinite(r-t)})(t)&&(t=t.filter(e=>Number.isFinite(e.x)&&Number.isFinite(e.y)&&o(e)<=Ht/2));let[s,c,l,u]=[1/0,1/0,-1/0,-1/0];for(let e of t)s=Math.min(s,Math.floor(e.x-e.range)),c=Math.min(c,Math.floor(e.y-e.range)),l=Math.max(l,Math.ceil(e.x+e.range)),u=Math.max(u,Math.ceil(e.y+e.range));let d=e.bounds;if(d&&([s,c,l,u]=[Math.max(s,d[0]),Math.max(c,d[1]),Math.min(l,d[2]),Math.min(u,d[3])]),t.length&&(l-s)*(u-c)>Ht*Ht){let e=Ht/2;[s,c,l,u]=[Math.max(s,Math.floor(a[0]-e)),Math.max(c,Math.floor(a[1]-e)),Math.min(l,Math.ceil(a[0]+e)),Math.min(u,Math.ceil(a[1]+e))]}if(!t.length||l<=s||u<=c)return{x0:0,y0:0,w:0,h:0,bits:new Uint8Array};let[f,p]=[l-s,u-c],m=zt(e,s,c,f,p),h=new Uint8Array(f*p),g=null;if(r){g=new Uint8Array(f*p);for(let e of n)Bt(m,e,g)}let _=new Uint8Array(f*p);for(let e of t){_.fill(0),Bt(m,e,_);for(let e=0;e<f*p;e++)_[e]&&(!g||g[e])&&(h[e]=1)}return{x0:s,y0:c,w:f,h:p,bits:h}}function Wt(e,t,n){let[r,i]=[Math.abs(e),Math.abs(t)];switch(n){case`chebyshev`:return Math.max(r,i);case`alternating`:return Math.max(r,i)+Math.floor(Math.min(r,i)/2);case`euclidean`:return Math.hypot(r,i);case`manhattan`:return r+i}}var Gt={ft:[5280,`mi`],feet:[5280,`mi`],foot:[5280,`mi`],yd:[1760,`mi`],yds:[1760,`mi`],m:[1e3,`km`],meters:[1e3,`km`],metres:[1e3,`km`]};function H(e,t){let n=e*t.scale,r=t.unit,i=Gt[r.toLowerCase()];return i&&n>=i[0]&&([n,r]=[n/i[0],i[1]]),`${(n>=100?Math.round(n):Math.round(n*10)/10).toLocaleString()}${r?` ${r}`:``}`}function Kt(e,t,n){let r=H(e,n);if(t===null)return r;if(Math.abs(t)<1)return`${r} · level`;let i=Math.abs(t)/5;return`${r} horizontal
${H(i,n)} ${t>0?`up`:`down`}
${H(Math.hypot(e,i),n)} diagonal`}function qt(e,t,n){let[r,i]=[0,0];for(let a=1;a<e.length;a++){let[o,s]=e[a];t.flags(o,s)&4&&i++;let[c,l]=e[a-1];n&&o>=0&&s>=0&&c>=0&&l>=0&&o<n.nx&&c<n.nx&&s<n.ny&&l<n.ny&&(r+=Math.max(0,Dt(n,l*n.nx+c,s*n.nx+o)))}return[r>0?`climb ${r} ft`:``,i>0?`${i} difficult square${i===1?``:`s`}`:``].filter(Boolean).join(` · `)}function Jt(e,t,n){let[r,i]=[Math.abs(t[0]-e[0]),Math.abs(t[1]-e[1])],a=[],[o,s]=e,[c,l]=[Math.sign(t[0]-e[0]),Math.sign(t[1]-e[1])],u=r-i;for(;a.push([o,s]),o!==t[0]||s!==t[1];){let e=2*u;e>-i&&(u-=i,o+=c),e<r&&(u+=r,s+=l)}return{squares:a,dist:Wt(r,i,n.measure)}}function Yt(e){let t=Math.hypot(e.dx,e.dy);switch(e.kind){case`circle`:{let e=[];for(let n=0;n<64;n++)e.push(Math.cos(n/64*Math.PI*2)*t,Math.sin(n/64*Math.PI*2)*t);return e}case`rect`:return[0,0,e.dx,0,e.dx,e.dy,0,e.dy];case`line`:return[0,0,e.dx,e.dy];case`cone`:{if(t<1e-6)return[];let n=Math.atan2(e.dy,e.dx),r=Math.atan(.5),i=[0,0];for(let e=0;e<=16;e++){let a=n-r+2*r*e/16;i.push(Math.cos(a)*t,Math.sin(a)*t)}return i}}}function Xt(e,t){let n=Math.hypot(e.dx,e.dy);return e.kind===`circle`?`${H(n,t)} radius`:e.kind===`rect`?`${H(Math.abs(e.dx),t)} × ${H(Math.abs(e.dy),t)}`:H(n,t)}var Zt=`#version 300 es
in vec2 aPosition;
in vec2 aUV;

out vec2 vUV;
out vec2 vLocal;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

void main() {
    mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
    gl_Position = vec4((mvp * vec3(aPosition, 1.0)).xy, 0.0, 1.0);
    vUV = aUV;
    vLocal = aPosition;
}
`,Qt=512,$t=new Map,en=new Set,tn=new Set;function nn(e){return tn.add(e),()=>tn.delete(e)}function rn(e){return $t.get(e)||(en.has(e)||(en.add(e),on(e).then(t=>{$t.set(e,t),en.delete(e);for(let t of tn)t(e)})),null)}async function an(e,t=Qt){let n=typeof e==`string`?e:URL.createObjectURL(e);try{let e=new Image;e.decoding=`async`,e.src=n,await e.decode();let r=e.naturalWidth||t,i=e.naturalHeight||t,a=Math.min(1,t/Math.max(r,i)),o=document.createElement(`canvas`);o.width=Math.max(1,Math.round(r*a)),o.height=Math.max(1,Math.round(i*a));let s=o.getContext(`2d`);return s.imageSmoothingQuality=`high`,s.drawImage(e,0,0,o.width,o.height),o}finally{typeof e!=`string`&&URL.revokeObjectURL(n)}}async function on(e){try{let t=await ve(e);if(t)return l.from(await an(t))}catch{}return cn()}var sn=null;function cn(){if(sn)return sn;let e=document.createElement(`canvas`);e.width=e.height=64;let t=e.getContext(`2d`);return t.fillStyle=`rgba(120, 96, 70, 0.55)`,t.strokeStyle=`#2b241d`,t.lineWidth=3,t.setLineDash([6,4]),t.beginPath(),t.arc(32,32,28,0,Math.PI*2),t.fill(),t.stroke(),t.fillStyle=`#f5ecd6`,t.font=`bold 34px Georgia, serif`,t.textAlign=`center`,t.textBaseline=`middle`,t.fillText(`?`,32,34),sn=l.from(e),sn}var ln=null,un=null;function dn(){if(un)return un;let e=new Uint8Array(1048576);for(let t=0;t<262144;t++){let n=t^2654435769;n=Math.imul(n^n>>>16,2146121005),n=Math.imul(n^n>>>15,2221713035),n^=n>>>16,e[t*4]=n&255,e[t*4+1]=n>>>8&255}return un=e}function fn(){return ln||(ln=new i({resource:dn(),width:512,height:512,format:`rgba8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:`linear`,addressMode:`repeat`}),ln)}function pn(e,t,n=96){let r=Math.max(1,Math.ceil(Math.sqrt(t.length))),i=Math.max(1,Math.ceil(t.length/r)),a=new k,o=new Map;t.forEach(({key:e,node:t},i)=>{let s=t.getLocalBounds();if(s.width<=0||s.height<=0)return;let c=(n-4)/Math.max(s.width,s.height),[l,u]=[s.width*c,s.height*c],[d,f]=[i%r*n+(n-l)/2,Math.floor(i/r)*n+(n-u)/2];t.scale.set(c),t.position.set(d-s.x*c,f-s.y*c),a.addChild(t),o.set(e,[Math.floor(d),Math.floor(f),Math.ceil(l),Math.ceil(u)])});let s=e.extract.canvas({target:a,frame:new x(0,0,r*n,i*n),resolution:1,clearColor:[0,0,0,0]});return a.destroy({children:!0}),{canvas:s,rects:o}}var mn=new WeakMap;function hn(e,t,n){let r=mn.get(e);r||mn.set(e,r=new Map);let i=r.get(t);return i||(i=new Promise((e,t)=>{let r=()=>{try{e(n())}catch(e){t(e)}};`requestIdleCallback`in window?requestIdleCallback(r,{timeout:500}):setTimeout(r,0)}),r.set(t,i),i.catch(()=>r.delete(t))),i}function gn(e,t,n){let r=e.getContext(`2d`),i=t.rects.get(n);if(!r||(r.clearRect(0,0,e.width,e.height),!i))return!1;let[a,o,s,c]=i,l=Math.min(e.width/s,e.height/c),[u,d]=[s*l,c*l];return r.drawImage(t.canvas,a,o,s,c,(e.width-u)/2,(e.height-d)/2,u,d),!0}var _n=2048,vn=new WeakMap,yn={moveTo:2,lineTo:2,quadraticCurveTo:4,bezierCurveTo:6,arc:3,arcTo:5,circle:3,ellipse:4,rect:4,roundRect:5};function bn(e,t){let n=new Proxy(e,{get(e,r,i){let a=Reflect.get(e,r,i);if(typeof a!=`function`)return a;let o=String(r);return(...r)=>{let i=r;if(o in yn)i=r.map((e,n)=>n<yn[o]&&typeof e==`number`?e*t:e);else if(o===`poly`)i=[r[0].map(e=>e*t),...r.slice(1)];else if(o===`setTransform`&&typeof r[0]==`number`)i=r.map((e,n)=>n>=4&&typeof e==`number`?e*t:e);else if(o===`stroke`&&r[0]&&typeof r[0]==`object`){let e=r[0];i=[{...e,width:(e.width??1)*t},...r.slice(1)]}let s=a.apply(e,i);return s===e?n:s}}});return n}var U=class e{renderer;pages=[];frames=new Map;pending=[];constructor(e){this.renderer=e}static for(t){let n=vn.get(t);return n||vn.set(t,n=new e(t)),n}static drop(e){vn.delete(e)}get(e,t,n=[0,0]){let r=this.frames.get(e);if(r)return r;let i=new P;t(bn(i,64));let a=i.getLocalBounds();if(a.width<=0||a.height<=0)return i.destroy(),null;let[o,s]=[Math.floor(a.x)-2,Math.floor(a.y)-2],[c,u]=[Math.ceil(a.width)+4,Math.ceil(a.height)+4];if(c>_n-8||u>_n-8)return i.destroy(),null;let[d,f,p]=this.place(c,u);i.position.set(f-o,p-s),this.pending.push({page:d,g:i});let m={texture:new l({source:d.rt.source,frame:new x(f,p,c,u)}),x:o/64-n[0],y:s/64-n[1]};return this.frames.set(e,m),m}flush(){if(!this.pending.length)return;let e=new Map;for(let{page:t,g:n}of this.pending){let r=e.get(t);r?r.push(n):e.set(t,[n])}this.pending=[];for(let[t,n]of e){let e=new k;e.addChild(...n),this.renderer.render({container:e,target:t.rt,clear:!1,transform:new v}),t.rt.source.updateMipmaps(),e.destroy({children:!0})}}place(e,t){let n=Math.ceil(e/8)*8+8,r=Math.ceil(t/8)*8+8,i=this.pages[this.pages.length-1];if(i&&i.x+n>_n&&(i.x=0,i.y+=i.rowH,i.rowH=0),!i||i.y+r>_n){let e=ue.create({width:_n,height:_n,resolution:1,autoGenerateMipmaps:!0,mipLevelCount:4});this.renderer.render({container:new k,target:e,clear:!0}),i={rt:e,x:0,y:0,rowH:0},this.pages.push(i)}let a=[i,i.x,i.y];return i.x+=n,i.rowH=Math.max(i.rowH,r),a}};function xn(e,t,n,r,i){let a=0;n[0]=0,r[0]=-1/0,r[1]=1/0;for(let i=1;i<t;i++){if(e[i]>=0x56bc75e2d63100000&&e[n[a]]>=0x56bc75e2d63100000)continue;let t=(e[i]+i*i-(e[n[a]]+n[a]*n[a]))/(2*i-2*n[a]);for(;t<=r[a];)a--,t=(e[i]+i*i-(e[n[a]]+n[a]*n[a]))/(2*i-2*n[a]);a++,n[a]=i,r[a]=t,r[a+1]=1/0}a=0;for(let o=0;o<t;o++){for(;r[a+1]<o;)a++;let t=o-n[a];i[o]=t*t+e[n[a]]}}function Sn(e,t,n,r,i,a,o,s){let[c,l]=[(a+8)*r,(o+8)*r],u=new Float32Array(c*l).fill(0x56bc75e2d63100000),d=(i-4)*r;for(let r=0;r<l;r++){let i=r+d;if(!(i<0||i>=n))for(let n=0;n<c;n++){let a=n+d;a>=0&&a<t&&e[i*t+a]>.5&&(u[r*c+n]=0)}}for(let e=0;e<o;e++)for(let t=0;t<a;t++)if(s(t,e))for(let n=(e+4)*r;n<(e+4+1)*r;n++)u.fill(0,n*c+(t+4)*r,n*c+(t+4+1)*r);let f=Math.max(c,l),p=new Float32Array(f),m=new Float32Array(f),h=new Int32Array(f),g=new Float32Array(f+1);for(let e=0;e<c;e++){for(let t=0;t<l;t++)p[t]=u[t*c+e];xn(p,l,h,g,m);for(let t=0;t<l;t++)u[t*c+e]=m[t]}let _=new Uint8Array(c*l),v=255/(8*r);for(let e=0;e<l;e++){let t=e*c;for(let e=0;e<c;e++)p[e]=u[t+e];xn(p,c,h,g,m);for(let e=0;e<c;e++)_[t+e]=Math.min(255,Math.sqrt(m[e])*v+.5)|0}return _}var W={Plain:0,Dwarven:1,Ashlar:2,Basalt:3,Dusty:4,Brick:5,Marble:6,Earth:7,Natural:8},G=(e,t,n,r)=>({style:e,stone:t,coping:n,rock:r,feel:0,tint:[1,1,1]}),Cn=G(W.Plain,[.58,.55,.5],[.56,.53,.48],[.28,.27,.25]),wn=G(W.Dwarven,[.64,.56,.45],[.6,.52,.41],[.29,.26,.22]),Tn=G(W.Ashlar,[.52,.54,.57],[.5,.52,.55],[.27,.28,.3]),En=G(W.Basalt,[.4,.32,.34],[.38,.27,.27],[.27,.23,.24]),Dn=G(W.Dusty,[.68,.64,.56],[.7,.66,.58],[.42,.4,.36]),On=G(W.Dusty,[.72,.68,.58],[.74,.7,.6],[.42,.4,.36]),kn=G(W.Brick,[.5,.5,.42],[.46,.5,.36],[.26,.29,.23]),An=G(W.Marble,[.6,.59,.57],[.72,.7,.67],[.29,.29,.3]),jn=G(W.Earth,[.52,.45,.36],[.5,.41,.31],[.45,.38,.3]),Mn=G(W.Natural,[.58,.55,.5],[.56,.53,.48],[.29,.27,.245]),Nn=G(W.Natural,[.58,.55,.5],[.56,.53,.48],[.3,.26,.22]),Pn=G(W.Natural,[.58,.55,.5],[.56,.53,.48],[.26,.225,.21]),Fn={...G(W.Natural,[.58,.55,.5],[.56,.53,.48],[.6,.7,.8]),feel:4},In=[{rock:[.25,.3,.215],tint:[.9,1,.86]},{rock:[.36,.3,.215],tint:[1.1,.99,.8]},{rock:[.3,.325,.36],tint:[.92,.99,1.07]}];function Ln(e,t,n=0){let r=Rn(e,t);if(e!==`cave`||n<1||n>3||r.feel===4)return r;let i=In[n-1];return{...r,rock:i.rock,tint:i.tint,feel:n}}function Rn(e,t){if(e===`sewer`)return kn;switch(t){case`dwarven_hall`:return wn;case`collapsed_fortress`:return Tn;case`cult_sanctum`:return En;case`ossuary`:case`catacombs`:return On;case`crypt`:case`tomb`:return Dn;case`temple`:case`sunken_temple`:case`wizard_lair`:return An;case`goblin_warren`:case`bandit_hideout`:return jn;case`ice`:return Fn}return e===`catacombs`?On:e===`crypt`?Dn:e===`cave`?Mn:e===`mine`?Nn:e===`lava tube`?Pn:Cn}function zn(e){let t=0;for(let n=0;n<e.length;n++){let[r,i]=e[n],[a,o]=e[(n+1)%e.length];t+=r*o-a*i}return t}var Bn=e=>zn(e)<0?e.reverse():e;function Vn(e){let t=[];for(let n of e){let e=t[t.length-1];(!e||Math.abs(e[0]-n[0])+Math.abs(e[1]-n[1])>1e-6)&&t.push(n)}for(;t.length>1&&Math.abs(t[0][0]-t[t.length-1][0])+Math.abs(t[0][1]-t[t.length-1][1])<=1e-6;)t.pop();return t}function Hn(e,t){let n=e;for(let e=0;e<t.length&&n.length;e++){let r=t[e],i=t[(e+1)%t.length],a=e=>(i[0]-r[0])*(e[1]-r[1])-(i[1]-r[1])*(e[0]-r[0]),o=n;n=[];for(let e=0;e<o.length;e++){let t=o[e],r=o[(e+1)%o.length],[i,s]=[a(t),a(r)];if(i>=0&&n.push(t),i>=0!=s>=0){let e=i/(i-s);n.push([t[0]+(r[0]-t[0])*e,t[1]+(r[1]-t[1])*e])}}}return n.length>=3?n:[]}function Un(e,t,n){let[r,i]=[0,1],a=[t[0]-e[0],t[1]-e[1]];for(let t=0;t<n.length;t++){let o=n[t],s=n[(t+1)%n.length],c=(s[0]-o[0])*(e[1]-o[1])-(s[1]-o[1])*(e[0]-o[0]),l=(s[0]-o[0])*a[1]-(s[1]-o[1])*a[0];if(Math.abs(l)<1e-12){if(c<0)return null;continue}let u=-c/l;if(l>0?r=Math.max(r,u):i=Math.min(i,u),r>i)return null}return i-r>1e-6?[r,i]:null}function Wn(e,t,n){let r=[[0,1]];for(let i of n){let n=Un(e,t,i);n&&(r=r.flatMap(([e,t])=>{if(n[1]<=e||n[0]>=t)return[[e,t]];let r=[];return n[0]>e&&r.push([e,n[0]]),n[1]<t&&r.push([n[1],t]),r}))}let i=n=>[e[0]+(t[0]-e[0])*n,e[1]+(t[1]-e[1])*n];return r.filter(([e,t])=>t-e>1e-4).map(([e,t])=>[i(e),i(t)])}function Gn(e,t,n){let r=[e[0]-t[0]*4,e[1]-t[1]*4],i=[e[0]+t[0]*4,e[1]+t[1]*4],a=Un(r,i,n);if(!a)return null;let o=[r[0]+(i[0]-r[0])*a[1],r[1]+(i[1]-r[1])*a[1]];return{at:o,tangent:qn(o,n)}}function Kn(e,t,n){return Gn(e,[-t[0],-t[1]],n)}function qn(e,t,n=.35){let r=1/0,i=0,a=0,o=!1,s=t.length,c=e=>{let n=t[(e%s+s)%s],r=t[((e+1)%s+s)%s],i=Math.hypot(r[0]-n[0],r[1]-n[1])||1;return[(r[0]-n[0])/i,(r[1]-n[1])/i]};for(let n=0;n<s;n++){let c=t[n],l=t[(n+1)%s],[u,d]=[l[0]-c[0],l[1]-c[1]],f=u*u+d*d;if(f<1e-12)continue;let p=Math.max(0,Math.min(1,((e[0]-c[0])*u+(e[1]-c[1])*d)/f)),m=Math.hypot(c[0]+u*p-e[0],c[1]+d*p-e[1]);m<r&&([r,i,a,o]=[m,n,(p>.5?1-p:p)*Math.sqrt(f),p>.5])}let l=c(i),u=a>=n?null:c(o?i+1:i-1);if(!u)return l;let[d,f]=[l[0]+u[0],l[1]+u[1]],p=Math.hypot(d,f)||1;return[d/p,f/p]}function Jn(e,t){return qn(e,t.reduce((t,n)=>Math.abs(K(e,n))<Math.abs(K(e,t))?n:t,t[0]))}function Yn(e,t=.12){let n=[];for(let r=0;r<e.length;r++){let i=e[r],a=e[(r+1)%e.length],o=Math.hypot(a[0]-i[0],a[1]-i[1]);if(o<1e-9)continue;let s=Math.max(1,Math.ceil(o/t)),c=[(a[1]-i[1])/o,-(a[0]-i[0])/o];for(let e=0;e<s;e++){let t=[i[0]+(a[0]-i[0])*e/s,i[1]+(a[1]-i[1])*e/s],r=[i[0]+(a[0]-i[0])*(e+1)/s,i[1]+(a[1]-i[1])*(e+1)/s];n.push({a:t,b:r,out:c})}}return n}function Xn(e,t){let n=[],r=null;for(let i=0;i<e.length;i++){if(!t(i)){r=null;continue}r||(r=[e[i].a],n.push(r)),r.push(e[i].b)}if(n.length>1&&t(0)&&t(e.length-1)){let e=n.shift();n[n.length-1].push(...e.slice(1))}return n}function Zn(e){let[t,n,r]=[Math.min(e.a[0],e.b[0]),Math.min(e.a[1],e.b[1]),Math.max(e.a[0],e.b[0])];return t===r?[[t-1,n],[t,n]]:[[t,n-1],[t,n]]}function K(e,t){let n=1/0;for(let r=0;r<t.length;r++){let i=t[r],a=t[(r+1)%t.length],o=Math.hypot(a[0]-i[0],a[1]-i[1]);o<1e-12||(n=Math.min(n,((a[0]-i[0])*(e[1]-i[1])-(a[1]-i[1])*(e[0]-i[0]))/o))}return n}var Qn=(e,t)=>t.reduce((t,n)=>Math.max(t,K(e,n)),-1/0),$n=(e,t)=>t.map(t=>Hn(e,t)).filter(e=>e.length);function er(e,t,n){return tr(n.map(n=>Un(e,t,n)).filter(e=>!!e))}function tr(e){e.sort((e,t)=>e[0]-t[0]);let t=[];for(let n of e){let e=t[t.length-1];e&&n[0]<=e[1]+1e-6?e[1]=Math.max(e[1],n[1]):t.push([n[0],n[1]])}return t}function nr(e,t,n){let r=er([e[0]-t[0]*4,e[1]-t[1]*4],[e[0]+t[0]*4,e[1]+t[1]*4],n).map(([e,t])=>[(e-.5)*2*4,(t-.5)*2*4]);if(!r.length)return null;let i=(r.find(([e,t])=>e<=1e-6&&t>=-1e-6)??r.filter(([,e])=>e<0).pop()??r[0])[1],a=[e[0]+t[0]*i,e[1]+t[1]*i];return{t:i,tangent:qn(a,n.reduce((e,t)=>Math.abs(K(a,t))<Math.abs(K(a,e))?t:e,n[0]))}}function rr(e,t,n){let r=nr(e,t,n);if(r&&Math.abs(r.t)<=.75)return{at:[e[0]+t[0]*r.t,e[1]+t[1]*r.t],t:r.t,tangent:r.tangent};let i=null,a=1/0;for(let r of n)for(let o=0;o<r.length;o++){let s=r[o],c=r[(o+1)%r.length],[l,u]=[c[0]-s[0],c[1]-s[1]],d=l*l+u*u;if(d<1e-12)continue;let f=Math.max(0,Math.min(1,((e[0]-s[0])*l+(e[1]-s[1])*u)/d)),p=[s[0]+l*f,s[1]+u*f];if(n.some(e=>e!==r&&K(p,e)>1e-4))continue;let m=Math.hypot(p[0]-e[0],p[1]-e[1]);m<a&&(a=m,i={at:p,t:(p[0]-e[0])*t[0]+(p[1]-e[1])*t[1],tangent:qn(p,r)})}return i}var ir=(e,t)=>t.reduce((t,n)=>Math.abs(K(e,n))<Math.abs(K(e,t))?n:t,t[0]);function ar(e,t){let n=[e[1],-e[0]];return n[0]*t[0]+n[1]*t[1]<0?[-n[0],-n[1]]:n}function or(e,t,n,r,i){let a=Gn(t,n,e)??Gn(r,i,e);return a?Math.max(0,(t[0]-a.at[0])*n[0]+(t[1]-a.at[1])*n[1]):null}function sr(e,t,n){let r=Un(e,t,n);return r&&r[1]<1-1e-9?[e[0]+(t[0]-e[0])*r[1],e[1]+(t[1]-e[1])*r[1]]:t}function cr(e,t,n,r,i){let a=[-t[1],t[0]],o=e;for(let e=0;e<6;e++){let e=[-1,1].flatMap(e=>[-1,1].map(i=>[o[0]+t[0]*n*e+a[0]*r*i,o[1]+t[1]*n*e+a[1]*r*i])),s=-q-Math.min(...e.map(e=>Qn(e,i)));if(s<=1e-4)break;o=[o[0]+a[0]*(s+.001),o[1]+a[1]*(s+.001)]}return o}function lr(e,t){let n=[],r=e;for(let e=0;e<t.length&&r.length;e++){let i=t[e],a=t[(e+1)%t.length],o=ur(r,a,i);o.length&&n.push(o),r=ur(r,i,a)}return n}function ur(e,t,n){let r=e=>(n[0]-t[0])*(e[1]-t[1])-(n[1]-t[1])*(e[0]-t[0]),i=[];for(let t=0;t<e.length;t++){let n=e[t],a=e[(t+1)%e.length],[o,s]=[r(n),r(a)];if(o>=0&&i.push(n),o>=0!=s>=0){let e=o/(o-s);i.push([n[0]+(a[0]-n[0])*e,n[1]+(a[1]-n[1])*e])}}return i.length>=3&&Math.abs(zn(i))>1e-9?i:[]}function dr(e,t){let n=[];for(let r=0;r<e.length;r++){let i=e[r],a=e[(r+1)%e.length],o=Math.hypot(a[0]-i[0],a[1]-i[1]);if(o<1e-9)continue;let s=[(i[0]+a[0])/2,(i[1]+a[1])/2];t.some(t=>t!==e&&K(s,t)>1e-4)||n.push({a:i,b:a,n:[(a[1]-i[1])/o,-(a[0]-i[0])/o]})}return n}function fr(e,t,n){let r=e.map(e=>e[0]),i=e.map(e=>e[1]),a=2*Math.abs(n),o=[[Math.min(...r)-a,Math.min(...i)-a],[Math.max(...r)+a,Math.min(...i)-a],[Math.max(...r)+a,Math.max(...i)+a],[Math.min(...r)-a,Math.max(...i)+a]],s=new Set(dr(e,t).map(e=>e.a));for(let t=0;t<e.length&&o.length;t++){let r=e[t],i=e[(t+1)%e.length],a=Math.hypot(i[0]-r[0],i[1]-r[1]);if(a<1e-9)continue;let c=s.has(r)?n:0,l=[(i[1]-r[1])/a,-(i[0]-r[0])/a];o=ur(o,[r[0]+l[0]*c,r[1]+l[1]*c],[i[0]+l[0]*c,i[1]+l[1]*c])}return o}function pr(e){if(e.length<8)return!1;let t=[e.reduce((e,t)=>e+t[0],0)/e.length,e.reduce((e,t)=>e+t[1],0)/e.length],n=e.map(e=>Math.hypot(e[0]-t[0],e[1]-t[1])),r=Math.max(...n);return r>0&&r-Math.min(...n)<=.15*r}var q=.225,mr=e=>e!==void 0&&e>=3&&e<=6,hr=e=>e!==void 0&&e>=1&&e<=7,gr=1-Math.SQRT1_2,_r=e=>e>=3?.75:.5,vr=(e,t)=>{let n=Math.min(e/2,t-1);return n<1?0:n},yr=e=>Math.max(3,Math.min(16,Math.round(e*4)));function br(e,t){let[n,r,i,a]=e,[o,s]=[i-n,a-r];if(t===1){let e=Math.min(o,s)/2,t=[],c=yr(e),l=[[i-e,r+e,-Math.PI/2],[i-e,a-e,0],[n+e,a-e,Math.PI/2],[n+e,r+e,Math.PI]];for(let[n,r,i]of l)for(let a=0;a<=c;a++)t.push([n+e*Math.cos(i+a/c*(Math.PI/2)),r+e*Math.sin(i+a/c*(Math.PI/2))]);return Vn(t)}if(t===2||t===7){let e=t===2?gr*Math.min(o,s):_r(Math.min(o,s));return[[n+e,r],[i-e,r],[i,r+e],[i,a-e],[i-e,a],[n+e,a],[n,a-e],[n,r+e]]}if(mr(t)){let e=t===3||t===5?o:s,c=t===3||t===5?s:o,l=vr(e,c),u=[];if(l<=0)u.push([0,0],[e,0]);else{let t=2*yr(Math.max(l,e/2));for(let n=0;n<=t;n++){let r=Math.PI+n/t*Math.PI;u.push([e/2+e/2*Math.cos(r),l+l*Math.sin(r)])}}return u.push([e,c],[0,c]),Bn(Vn(u.map(([e,o])=>t===3?[n+e,r+o]:t===5?[n+e,a-o]:t===6?[n+o,r+e]:[i-o,r+e])))}return null}function xr(e,t,n){let r=e.rooms.map(()=>null);for(let i=0;i<n;i++)for(let n=0;n<t;n++){let a=e.cells[i*t+n];if(a<0)continue;let o=r[a];r[a]=o?[Math.min(o[0],n),Math.min(o[1],i),Math.max(o[2],n+1),Math.max(o[3],i+1)]:[n,i,n+1,i+1]}return r}var Sr=q+.5,Cr=[[-1,0],[0,-1],[1,0],[0,1]],wr=new Set([`corridor`,`minstrels' gallery`,`mural passage`]),Tr=[...Cr,[-1,-1],[1,-1],[1,1],[-1,1]],Er=e=>e.flatMap(e=>e),Dr=(e,t)=>[[e,t],[e+1,t],[e+1,t+1],[e,t+1]];function Or(e,t){let n=new Uint8Array(t);for(let r of e)for(let e=0;e<t;e++)r.cells[e]>=0&&(n[e]=1);return n}function kr(e,t,n,r){let i=e.rooms[n].shape;return br(r,i)}function Ar(e){let t=e.flat().slice().sort((e,t)=>e[0]-t[0]||e[1]-t[1]);if(t.length<3)return null;let n=(e,t,n)=>(t[0]-e[0])*(n[1]-e[1])-(t[1]-e[1])*(n[0]-e[0]),r=e=>{let t=[];for(let r of e){for(;t.length>=2&&n(t[t.length-2],t[t.length-1],r)<=1e-9;)t.pop();t.push(r)}return t.pop(),t},i=Bn([...r(t),...r(t.slice().reverse())]);if(i.length<3)return null;let a=i.map(e=>e[0]),o=i.map(e=>e[1]);for(let t=Math.min(...o);t<=Math.max(...o);t+=.1)for(let n=Math.min(...a);n<=Math.max(...a);n+=.1)if(K([n,t],i)>.02&&!e.some(e=>K([n,t],e)>=-.02))return null;return i}function jr(e,t,n,r,i){let a=!!e?.length;if(!t.rooms.some(e=>hr(e.shape))&&!a)return null;let o=a?e.map(e=>e.map(e=>[e[0],e[1]])):[[[0,0],[n,0],[n,r],[0,r]]],s=n+2,c=(e,t)=>(t+1)*s+e+1,l=(e,i)=>e>=0&&i>=0&&e<n&&i<r?t.cells[i*n+e]:-1,u=xr(t,n,r),d=a?o.map(e=>fr(e,o,-q)):o,f=(e,i)=>{for(let a=0;a<r;a++)for(let r=0;r<n;r++){let o=K([r+.5,a+.5],e);if(t.cells[a*n+r]===i?o<=0:o>.03)return!1}return!0},p=t.rooms.map((e,r)=>{let i=u[r],s=i&&hr(e.shape)?kr(t,n,r,i):null;if(!s)return null;if(a&&e.shape===1&&o.length===1&&pr(o[0])&&d[0].length&&f(d[0],r))return d[0];let c=d.map(e=>Hn(s,e)).filter(e=>e.length).sort((e,t)=>Math.abs(zn(t))-Math.abs(zn(e)));if(c.length>1){let e=Ar(d.filter(e=>Hn(s,e).length)),t=e?Hn(s,e):[];if(t.length)return t}return c[0]??null});t.rooms.forEach((e,n)=>{let r=u[n];if(e.kind!==`open to below`||e.shape!==1||!r||!p[n])return;let i=new Map;for(let e=r[1];e<r[3];e++)for(let t=r[0];t<r[2];t++)if(l(t,e)===n)for(let[r,a]of Cr){let o=l(t+r,e+a);o>=0&&o!==n&&i.set(o,(i.get(o)??0)+1)}let a=[...i].sort((e,t)=>t[1]-e[1]||e[0]-t[0])[0]?.[0];if(a===void 0||!wr.has(t.rooms[a].kind)||p[a])return;let o=br([r[0]-1,r[1]-1,r[2]+1,r[3]+1],1),s=o?d.map(e=>Hn(o,e)).filter(e=>e.length).sort((e,t)=>Math.abs(zn(t))-Math.abs(zn(e)))[0]:void 0;s&&(p[a]=s)});let m=e=>e>=0&&!!p[e]?.length,h=new Int32Array(s*(r+2)).fill(-1);for(let e=-1;e<=r;e++)for(let t=-1;t<=n;t++){let s=l(t,e);if(s>=0){h[c(t,e)]=s;continue}let u=t>=0&&e>=0&&t<n&&e<r;if(!a){u&&i[e*n+t]&&(h[c(t,e)]=-2);continue}if(!$n(Dr(t,e),o).length)continue;let d=-2;if(!(u&&i[e*n+t])&&Qn([t+.5,e+.5],o)<=Sr)for(let[n,r]of Tr){let i=l(t+n,e+r);if(i>=0){d=m(i)?-2:i;break}}h[c(t,e)]=d}let g=new Int32Array(n*r).fill(-1),_=new Map;for(let e of t.doors){if(e.kind===`secret`)continue;let[t,r]=Zn(e),[i,a]=[l(t[0],t[1]),l(r[0],r[1])];i<0||a<0||i===a||(m(i)&&!m(a)&&_.set(t[1]*n+t[0],a),m(a)&&!m(i)&&_.set(r[1]*n+r[0],i))}for(let e=0;e<r;e++)for(let t=0;t<n;t++){let r=l(t,e);if(!m(r))continue;let i=_.get(e*n+t)??-2;for(let[n,a]of Cr){let o=l(t+n,e+a);if(i!==-2)break;o>=0&&o!==r&&!m(o)&&(i=o)}g[e*n+t]=i}return{nx:n,ny:r,parts:o,real:a,owner:h,under:g,polys:p}}function Mr(e,t,n){let r=!1;for(let{seg:n,clip:i}of t){let[t,a]=n;for(let n of er(t,a,i))e.moveTo(t[0]+(a[0]-t[0])*n[0],t[1]+(a[1]-t[1])*n[0]).lineTo(t[0]+(a[0]-t[0])*n[1],t[1]+(a[1]-t[1])*n[1]),r=!0}r&&e.stroke(n)}var Nr=(e,t,n)=>t>=0&&n>=0&&t<e.nx&&n<e.ny&&e.under[n*e.nx+t]!==-1?e.under[n*e.nx+t]:e.owner[(n+1)*(e.nx+2)+t+1];function Pr(e,t,n,r,i,a){let{nx:o,ny:s,parts:c,polys:l}=t,u=[];for(let n=-1;n<=s;n++)for(let a=-1;a<=o;a++){let o=Nr(t,a,n);if(!(o<0||r(o)<0)){for(let t of $n(Dr(a,n),c))e.poly(Er(t)).fill(r(o));for(let e of i(a,n,r(o)))u.push({seg:e,clip:c})}}Mr(e,u,{width:.03,color:0,alpha:.25});let d=[];l.forEach((t,n)=>{if(!t?.length||r(n)<0)return;e.poly(Er(t)).fill(r(n));let a=t.map(e=>e[0]),o=t.map(e=>e[1]);for(let e=Math.floor(Math.min(...o));e<Math.ceil(Math.max(...o));e++)for(let o=Math.floor(Math.min(...a));o<Math.ceil(Math.max(...a));o++)for(let a of i(o,e,r(n)))d.push({seg:a,clip:[t]})}),Mr(e,d,{width:.03,color:0,alpha:.25}),a?.();let f=[];for(let e=0;e<s;e++)for(let t=0;t<o;t++){let i=n.cells[e*o+t];if(i<0||r(i)<0)continue;let a=l[i]?.length?[l[i]]:c,s=Dr(t,e);for(let e=0;e<4;e++)f.push({seg:[s[e],s[(e+1)%4]],clip:a})}Mr(e,f,{width:.04,color:0,alpha:.35})}function Fr(e,t,n){let r=Ir(t);for(let t of r)e.poly(Er(t));r.length&&e.fill(n)}function Ir(e){let{nx:t,ny:n,parts:r,polys:i}=e,a=i.filter(e=>!!e?.length).map(e=>({p:e,box:[Math.min(...e.map(e=>e[0])),Math.min(...e.map(e=>e[1])),Math.max(...e.map(e=>e[0])),Math.max(...e.map(e=>e[1]))]})),o=[];for(let i=-1;i<=n;i++)for(let n=-1;n<=t;n++){if(Nr(e,n,i)!==-2)continue;let t=$n(Dr(n,i),r);for(let{p:e,box:r}of a)r[0]<n+1&&r[2]>n&&r[1]<i+1&&r[3]>i&&(t=t.flatMap(t=>lr(t,e)));o.push(...t)}return o}function Lr(e,t,n){let{nx:r,ny:i}=e,a=e=>e[0]>=0&&e[1]>=0&&e[0]<r&&e[1]<i?t.cells[e[1]*r+e[0]]:-1,[o,s]=Zn(n),[c,l]=[a(o),a(s)],u=[(n.a[0]+n.b[0])/2,(n.a[1]+n.b[1])/2],d=[s[0]-o[0],s[1]-o[1]],f=Math.hypot(n.b[0]-n.a[0],n.b[1]-n.a[1])*.45,p={rooms:n.rooms,out:c<0||l<0,a:n.a,b:n.b,moved:!1},m=(e,t)=>({a:[e[0]-t[0]*f,e[1]-t[1]*f],b:[e[0]+t[0]*f,e[1]+t[1]*f]});if(p.out){let t=c>=0?c:l,n=c>=0?d:[-d[0],-d[1]],r=[Math.abs(n[1]),Math.abs(n[0])],i=e.real?rr(u,n,e.parts):{at:u,t:0,tangent:r};if(!i)return p;let a=i.tangent,o=i.at,s=ar(a,n),h=e.polys[t]?.length?or(e.polys[t],o,s,u,n):null;if(h!==null&&(e.real||h>.245)){let n=[o[0]-s[0]*h,o[1]-s[1]*h],r=[o[0]+s[0]*q,o[1]+s[1]*q],i=m(n,a),c=m(r,a);if(e.real){let t=fr(ir(o,e.parts),e.parts,q);[c.a,c.b]=[sr(i.a,c.a,t),sr(i.b,c.b,t)]}return{...p,...i,moved:!0,outer:o,inner:n,threshold:[i.a,i.b,c.b,c.a],room:t}}return Math.hypot(o[0]-u[0],o[1]-u[1])<1e-6?{...p,outer:o}:{...p,...m(cr(o,a,f,.06,e.parts),a),moved:!0,outer:o}}let h=Rr(e.polys,t,n,r,i);return h?{...p,...h,moved:!0,inner:[(h.a[0]+h.b[0])/2,(h.a[1]+h.b[1])/2]}:p}function Rr(e,t,n,r,i){let a=e=>e[0]>=0&&e[1]>=0&&e[0]<r&&e[1]<i?t.cells[e[1]*r+e[0]]:-1,[o,s]=Zn(n),[c,l]=[a(o),a(s)];if(c<0||l<0)return null;let u=[(n.a[0]+n.b[0])/2,(n.a[1]+n.b[1])/2],d=[s[0]-o[0],s[1]-o[1]],f=Math.hypot(n.b[0]-n.a[0],n.b[1]-n.a[1])*.45;for(let[t,n]of[[l,d],[c,[-d[0],-d[1]]]]){let r=e[t],i=r?.length?Kn(u,n,r):null;if(i)return{a:[i.at[0]-i.tangent[0]*f,i.at[1]-i.tangent[1]*f],b:[i.at[0]+i.tangent[0]*f,i.at[1]+i.tangent[1]*f]}}return null}function zr(e,t){let{nx:n,ny:r}=e,i=e=>e[0]>=0&&e[1]>=0&&e[0]<n&&e[1]<r?t.cells[e[1]*n+e[0]]:-1,a=[];for(let[n,r,o,s]of t.windows){let[t,c]=Zn({a:[n,r],b:[o,s]}),[l,u]=i(t)>=0?[i(t),[c[0]-t[0],c[1]-t[1]]]:[i(c),[t[0]-c[0],t[1]-c[1]]];if(l<0)continue;let d=[(n+o)/2,(r+s)/2],f=e.real?rr(d,u,e.parts):{at:d,t:0,tangent:[Math.abs(u[1]),Math.abs(u[0])]};if(!f)continue;let p=f.at,m=f.tangent,h=ar(m,u),g=e.polys[l],_=Math.max(q,(g?.length?or(g,p,h,d,u):null)??.225);if(_+.225<.5){a.push({at:cr(p,m,.3,.09,e.parts),tangent:m,room:l});continue}let v=[p[0]-h[0]*_,p[1]-h[1]*_],y=[p[0]-h[0]*q,p[1]-h[1]*q],b=[[v[0]-m[0]*.32,v[1]-m[1]*.32],[v[0]+m[0]*.32,v[1]+m[1]*.32],[y[0]+m[0]*.16,y[1]+m[1]*.16],[y[0]-m[0]*.16,y[1]-m[1]*.16]];a.push({at:p,tangent:m,room:l,inner:v,splay:b})}return a}function Br(e,t,n,r,i,a,o,s){let{nx:c,ny:l,owner:u,polys:d,parts:f}=t,p=c+2,m=(e,t)=>u[(t+1)*p+e+1],h=(e,t)=>e>=0&&t>=0&&e<c&&t<l&&n.cells[t*c+e]>=0&&!!d[n.cells[t*c+e]]?.length,g=new Set(n.doors.map(e=>`${Math.min(e.a[0],e.b[0])},${Math.min(e.a[1],e.b[1])},${e.a[0]===e.b[0]?`v`:`h`}`));for(let e of n.walls){if(!e.rail)continue;let t=e.a[0]===e.b[0],[n,r]=t?[Math.min(e.a[1],e.b[1]),Math.max(e.a[1],e.b[1])]:[Math.min(e.a[0],e.b[0]),Math.max(e.a[0],e.b[0])];for(let i=n;i<r;i++)g.add(t?`${e.a[0]},${i},v`:`${i},${e.a[1]},h`)}let _=d.filter(e=>!!e?.length),v=e=>e>=0&&n.rooms[e]?.kind===`roof`,y=[];for(let e of[!1,!0]){let t=e?l:c;for(let n=0;n<=(e?c:l);n++){let r=null;for(let i=-1;i<=t+1;i++){let[a,o]=e?[[n-1,i],[n,i]]:[[i,n-1],[i,n]],s=e?`${n},${i},v`:`${i},${n},h`;i<=t&&(()=>{let[e,t]=[m(a[0],a[1]),m(o[0],o[1])];return e>=0&&t>=0&&e!==t&&!v(e)&&!v(t)&&!h(a[0],a[1])&&!h(o[0],o[1])&&!g.has(s)})()?r??=i:r!==null&&(y.push(e?[[n,r],[n,i]]:[[r,n],[i,n]]),r=null)}}}let b=0;for(let[t,n]of y){let r=e=>[t[0]+(n[0]-t[0])*e,t[1]+(n[1]-t[1])*e];for(let i of er(t,n,f))for(let[t,n]of Wn(r(i[0]),r(i[1]),_))e.moveTo(t[0],t[1]).lineTo(n[0],n[1]),b++}b&&e.stroke({width:a,color:s,cap:`square`});let x=(e,t)=>t.some(t=>Math.hypot(t[0]-e[0],t[1]-e[1])<.45),S=e=>[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2],C=t=>{e.moveTo(t[0][0],t[0][1]);for(let n of t.slice(1))e.lineTo(n[0],n[1]);b++};if(b=0,d.forEach((a,o)=>{if(!a?.length||n.rooms[o].kind===`open to below`)return;let s=[...r.filter(e=>(e.rooms[0]===o||e.rooms[1]===o)&&e.inner).map(e=>({at:e.inner,half:Math.hypot(e.b[0]-e.a[0],e.b[1]-e.a[1])/2})),...i.filter(e=>e.room===o&&e.inner).map(e=>({at:e.inner,half:.32}))],c=e=>s.some(t=>Math.hypot(t.at[0]-e[0],t.at[1]-e[1])<t.half),l=Yn(a),u=e=>!c(S(l[e]))&&(t.real||Qn(S(l[e]),f)>.02);if(l.every((e,t)=>u(t)))e.poly(Er(a),!0),b++;else for(let e of Xn(l,u))C(e)}),b&&e.stroke({width:a,color:s,cap:`butt`,join:`round`}),!t.real)return[];b=0;let w=r.filter(e=>e.out&&e.outer).map(e=>e.outer),T=n.walls.filter(e=>e.arcade),E=e=>T.some(t=>{let[n,r]=[t.b[0]-t.a[0],t.b[1]-t.a[1]],i=n*n+r*r||1,a=((e[0]-t.a[0])*n+(e[1]-t.a[1])*r)/i;return a>0&&a<1&&Math.abs((e[0]-t.a[0])*r-(e[1]-t.a[1])*n)/Math.sqrt(i)<.6}),D=[];return f.forEach((t,n)=>{let r=Yn(t),i=e=>!x(S(r[e]),w)&&!E(S(r[e]))&&!f.some((t,i)=>i!==n&&K(S(r[e]),t)>1e-4);if(r.every((e,t)=>i(t)))e.poly(Er(t),!0),b++;else for(let e of Xn(r,i))C(e);D.push(...r.filter((e,t)=>i(t)))}),b&&e.stroke({width:o,color:s,cap:`butt`,join:`miter`}),D}function Vr(e,t,n,r){e.poly(Er(t)).fill(n),e.moveTo(t[1][0],t[1][1]).lineTo(t[2][0],t[2][1]).moveTo(t[3][0],t[3][1]).lineTo(t[0][0],t[0][1]).stroke({width:.08,color:r,cap:`butt`})}function Hr(e,t,n){let r=new Uint8Array(t*n),i=[],a=e.rooms.map(()=>null);if(!e.rooms.some(e=>e.shape))return{index:r,shapes:i,caverns:!1,polys:a};let o=xr(e,t,n),s=new Map;e.rooms.forEach((n,r)=>{let c=o[r];c&&hr(n.shape)&&i.length<253?(s.set(r,i.length+1),i.push({box:c,shape:n.shape,room:r}),a[r]=kr(e,t,r,c)):c&&n.shape===255&&s.set(r,255)});let c=!1;for(let i=0;i<t*n;i++){let t=e.cells[i],n=t>=0?s.get(t):void 0;n!==void 0&&(r[i]=n,c||=n===255)}for(let i of e.doors)if(i.kind!==`secret`)for(let[e,a]of Zn(i))e>=0&&a>=0&&e<t&&a<n&&r[a*t+e]!==255&&(r[a*t+e]=0);let l=new Uint8Array(t*n);for(let e=0;e<t*n;e++){if(r[e]===0)continue;let[i,a]=[e%t,Math.floor(e/t)];for(let e=Math.max(0,a-1);e<=Math.min(n-1,a+1);e++)for(let n=Math.max(0,i-1);n<=Math.min(t-1,i+1);n++)l[e*t+n]=1}for(let e=0;e<t*n;e++)l[e]&&r[e]===0&&(r[e]=254);return{index:r,shapes:i,caverns:c,polys:a}}var Ur=e=>e===0||e===254,Wr=new WeakMap;function Gr(e,t){let n=Wr.get(e);n||Wr.set(e,n=new Map);let r=n.get(t);if(r)return r;let i=e.levels[t];if(/^[uwk]:/.test(e.id)){let{polys:t}=Hr(i,e.nx,e.ny),n=t.some(e=>e)?i.doors.map(n=>n.kind===`secret`?Rr(t,i,n,e.nx,e.ny):null):[];r={plan:null,doors:n.some(e=>e)?i.doors.map((e,t)=>{let r=n[t];return r?{a:r.a,b:r.b,rooms:e.rooms,out:!1,moved:!0,inner:[(r.a[0]+r.b[0])/2,(r.a[1]+r.b[1])/2]}:{a:e.a,b:e.b,rooms:e.rooms,out:!1,moved:!1}}):null}}else{let n=e.outlines;if(e.outlines&&e.outlines.length>1){let t=e.outlines.map(e=>e.map(e=>[e[0],e[1]])),r=new Set;for(let n=0;n<e.nx*e.ny;n++){if(i.cells[n]<0)continue;let a=[n%e.nx+.5,Math.floor(n/e.nx)+.5],o=t.flatMap((e,t)=>K(a,e)>0?[t]:[]);o.length===1&&r.add(o[0])}n=e.outlines.filter((e,t)=>r.has(t))}let a=jr(n,i,e.nx,e.ny,Or(e.levels,e.nx*e.ny)),o=a?i.doors.map(e=>Lr(a,i,e)):null,s=qr(e,t);o?.forEach((e,t)=>{if(!s||i.doors[t].kind!==`front`)return;let n=[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2],[r,a]=s.south,c=Math.hypot(n[0]-s.c[0],n[1]-s.c[1])||1;if(((n[0]-s.c[0])*r+(n[1]-s.c[1])*a)/c<.6)return;let l=[s.c[0]+r*s.r,s.c[1]+a*s.r],[u,d]=[l[0]-n[0],l[1]-n[1]],f=Math.hypot(e.b[0]-e.a[0],e.b[1]-e.a[1])/2,p=(e.b[0]-e.a[0])*-a+(e.b[1]-e.a[1])*r>=0?[-a,r]:[a,-r],m=e=>[e[0]+u,e[1]+d];o[t]={...e,a:[l[0]-p[0]*f,l[1]-p[1]*f],b:[l[0]+p[0]*f,l[1]+p[1]*f],outer:e.outer&&m(e.outer),inner:e.inner&&m(e.inner),threshold:e.threshold?.map(m),moved:!0}}),r={plan:a,doors:o}}return n.set(t,r),r}var Kr=new Set([`yurt`,`felt hall`,`felt sanctuary`,`felt common room`]);function qr(e,t){let n=e.levels[t],r=e.outlines;if(!n||!r||r.length!==1||r[0].length<8||!n.rooms.some(e=>Kr.has(e.kind)))return null;let i=r[0].length,a=[r[0].reduce((e,t)=>e+t[0],0)/i,r[0].reduce((e,t)=>e+t[1],0)/i],o=r[0].reduce((e,t)=>e+Math.hypot(t[0]-a[0],t[1]-a[1]),0)/i,s=Math.hypot(e.axis[1],e.across[1])||1;return{c:a,r:o,south:[e.axis[1]/s,e.across[1]/s]}}var Jr=(e,t)=>Gr(e,t).plan,Yr=(e,t)=>Gr(e,t).doors;function Xr(e,t,n){let r=e.walls.filter(e=>!e.exterior);if(!e.rooms.some(e=>hr(e.shape)))return r.map(e=>[e.a,e.b]);let{index:i,polys:a}=Hr(e,t,n),o=(r,o)=>r>=0&&o>=0&&r<t&&o<n&&!Ur(i[o*t+r])&&i[o*t+r]!==255?a[e.cells[o*t+r]]:null,s=[];for(let e of r){let t=e.a[0]===e.b[0],[n,r]=t?[Math.min(e.a[1],e.b[1]),Math.max(e.a[1],e.b[1])]:[Math.min(e.a[0],e.b[0]),Math.max(e.a[0],e.b[0])];for(let i=n;i<r;i++){let[n,r]=t?[[e.a[0],i],[e.a[0],i+1]]:[[i,e.a[1]],[i+1,e.a[1]]],[a,c]=t?[o(e.a[0]-1,i),o(e.a[0],i)]:[o(i,e.a[1]-1),o(i,e.a[1])];if(!a||!c){s.push([n,r]);continue}for(let[e,t]of er(n,r,[a,c]))s.push([[n[0]+(r[0]-n[0])*e,n[1]+(r[1]-n[1])*e],[n[0]+(r[0]-n[0])*t,n[1]+(r[1]-n[1])*t]])}}return s}var Zr=5,Qr=1,$r=.9,ei=.1,ti=.25,J=2,ni=63.5,ri=.4,ii=20,ai={water_shallow:0,water_deep:1,chasm:2,rubble_slope:3},oi=new Set([`dungeon`,`crypt`,`catacombs`,`deep dungeons`]);function si(e){return e===`cave`?1:e===`lava tube`?.85:e===`mine`?.45:oi.has(e)?0:.06}function ci(e,t,n){return t===`ring corridor`?10:t===`sewer tunnel`||t===`service passage`?3:n===`ice`?9:e===`cave`?0:n===`goblin_warren`||n===`bandit_hideout`?2:t===`boss chamber`?5:n===`cult_sanctum`?6:n===`temple`||n===`sunken_temple`||n===`wizard_lair`?7:n===`dwarven_hall`||n===`labyrinth`?8:e===`lava tube`?1:e===`mine`?2:e===`crypt`||e===`catacombs`?11:4}function li(e,t,n){let r=new Uint8Array(t*n);if(!Et(e,t,n))return r;for(let i=0;i<e.rooms.length;i++){if(e.rooms[i].kind!==`boss chamber`)continue;let a=0,o=0;for(let r=0;r<t*n;r++)e.cells[r]===i&&(a++,Mt(e,t,n,r%t,Math.floor(r/t))>3&&o++);if(o*5<=a*2)continue;let s=a=>e.cells[a]===i&&!r[a]&&Mt(e,t,n,a%t,Math.floor(a/t))<.5,c=new Uint8Array(t*n);for(let e=0;e<t*n;e++){if(c[e]||!s(e))continue;let i=[e];c[e]=1;for(let e=0;e<i.length;e++){let r=i[e],a=r%t;for(let e of[a>0?r-1:-1,a+1<t?r+1:-1,r-t,r+t])e>=0&&e<t*n&&!c[e]&&s(e)&&(c[e]=1,i.push(e))}if(i.length>=4)for(let e of i)r[e]=1}}return r}function Y(e,t,n,r){let i=Math.ceil(r*2.5),a=new Float32Array(2*i+1),o=0;for(let e=-i;e<=i;e++)o+=a[e+i]=Math.exp(-(e*e)/(2*r*r));for(let e=0;e<a.length;e++)a[e]/=o;let s=new Float32Array(t*n),c=new Float32Array(t*n);for(let r=0;r<n;r++){let n=r*t;for(let r=0;r<t;r++){let o=0;if(r>=i&&r<t-i)for(let t=-i;t<=i;t++)o+=e[n+r+t]*a[t+i];else for(let s=-i;s<=i;s++){let c=r+s;c>=0&&c<t&&(o+=e[n+c]*a[s+i])}s[n+r]=o}}for(let e=0;e<n;e++){let r=e*t;for(let o=-i;o<=i;o++){let l=e+o;if(l<0||l>=n)continue;let u=l*t,d=a[o+i];for(let e=0;e<t;e++)c[r+e]+=s[u+e]*d}}return c}function ui(e,t,n,r=0){let i=t+2*r,a=new Uint8Array(i*(n+2*r)),o=(e,o)=>{for(let s=Math.max(-r,o-1);s<=Math.min(n+r-1,o+1);s++)for(let n=Math.max(-r,e-1);n<=Math.min(t+r-1,e+1);n++)a[(s+r)*i+n+r]=1};for(let r=0;r<n;r++)for(let n=0;n<t;n++)e.cells[r*t+n]>=0&&o(n,r);for(let[t,n]of e.rim??[])o(t,n);return a}function di([e,t,n,r,i],a,o){let[s,c]=[n-e,r-t],l=Math.max(0,Math.min(1,((a-e)*s+(o-t)*c)/Math.max(s*s+c*c,1e-9))),[u,d]=[a-e-l*s,o-t-l*c],f=Math.max(Math.hypot(u,d),1e-9);return[f-i,u/f,d/f]}function fi(e,t,n){let r=e[0],i=e[1],a=e[2]-r,o=e[3]-i,s=Math.max(0,Math.min(1,((t-r)*a+(n-i)*o)/Math.max(a*a+o*o,1e-9))),c=t-r-s*a,l=n-i-s*o;return Math.max(Math.hypot(c,l),1e-9)-e[4]<=0}var pi=64,mi=.1,hi=2.6,gi=.5;function _i(e,t,n,r,i,a){let[o,s]=[n+2*i,r+2*i],c=Array(o*s);e.forEach((e,t)=>{let a=e[4]+1,s=Math.max(-i,Math.floor(Math.min(e[0],e[2])-a)),l=Math.max(-i,Math.floor(Math.min(e[1],e[3])-a)),u=Math.min(n+i-1,Math.floor(Math.max(e[0],e[2])+a)),d=Math.min(r+i-1,Math.floor(Math.max(e[1],e[3])+a));for(let n=l;n<=d;n++)for(let r=s;r<=u;r++){if(di(e,r+.5,n+.5)[0]>.75)continue;let a=(n+i)*o+r+i;(c[a]??=[]).push(t)}});let l=(e,t)=>{let[n,r]=[Math.floor(e)+i,Math.floor(t)+i];return n>=0&&r>=0&&n<o&&r<s?r*o+n:-1},u=(n,r)=>{let i=l(n,r);i>=0&&t[i]&&!(c[i]??[]).some(t=>di(e[t],n,r)[0]<-.001)&&a(n,r)};e.forEach(([e,t,n,r,i])=>{let a=Math.hypot(n-e,r-t);if(a>1e-6){let[o,s]=[-(r-t)/a*i,(n-e)/a*i],c=Math.ceil(a/ei);for(let i=0;i<=c;i++){let[a,l]=[e+(n-e)*i/c,t+(r-t)*i/c];u(a+o,l+s),u(a-o,l-s)}}let o=Math.ceil(2*Math.PI*i/ei);for(let a=0;a<o;a++){let s=a*2*Math.PI/o,[c,l]=[Math.cos(s)*i,Math.sin(s)*i];u(e+c,t+l),u(n+c,r+l)}});let d=Math.round(1/ei);for(let n=0;n+1<s;n++)for(let r=0;r+1<o;r++){let s=n*o+r;for(let[l,u,f]of[[s+1,1,0],[s+o,0,1]]){if(t[s]===t[l])continue;let[o,p]=u?[r+1-i,n-i]:[r-i,n+1-i],m=c[t[s]?s:l]??[];for(let t=0;t<=d;t++){let[n,r]=[o+f*t/d,p+u*t/d];m.some(t=>di(e[t],n,r)[0]<0)&&a(n,r)}}}}function vi(e,t,n,r,i,a,o,s){let c=1.025,[l,u]=[c*c,5],d=new Float32Array(o*s).fill(l);_i(e,t,r,i,a,(e,t)=>{let[n,r]=[(e+a)*4-.5,(t+a)*4-.5],[i,c]=[Math.max(0,Math.round(n)-u),Math.max(0,Math.round(r)-u)],[l,f]=[Math.min(o-1,Math.round(n)+u),Math.min(s-1,Math.round(r)+u)];for(let e=c;e<=f;e++){let t=(e-r)/4;for(let r=i;r<=l;r++){let i=(r-n)/4,a=i*i+t*t;a<d[e*o+r]&&(d[e*o+r]=a)}}});let f=new Float32Array(o*s);for(let e=0;e<o*s;e++)n[e]&&(f[e]=Math.max(0,Math.min(1,(Math.sqrt(d[e])-$r)/ti+.5)));return f}function yi(e,t,n,r,i,a){let o=(e.paths??[]).filter(e=>di(e,i,a)[0]<=2.65),s=(i,a)=>{let[s,c]=[Math.floor(i),Math.floor(a)];if(s>=0&&c>=0&&s<t&&c<n){let n=c*t+s,i=e.cells[n];if(i>=0&&e.rooms[i].kind!==`sewer tunnel`)return!0;if(!r[n])return!1}for(let e of o)if(fi(e,i,a))return!0;return!1},c=s(i,a),l=null,u=hi;for(let e=0;e<pi;e++){let[r,d]=[Math.cos(e/pi*Math.PI*2),Math.sin(e/pi*Math.PI*2)];for(let e=mi;e<u;e+=mi){if(s(i+r*e,a+d*e)===c)continue;let[f,p]=[e-mi,e];for(let e=0;e<6;e++){let e=(f+p)/2;s(i+r*e,a+d*e)===c?f=e:p=e}let[m,h]=[i+r*p,a+d*p],g=null;for(let e of o){let t=di(e,m,h);Math.abs(t[0])<.02&&(!g||Math.abs(t[0])<Math.abs(g[0]))&&(g=t)}g&&p<u&&m>=gi&&h>=gi&&m<=t-gi&&h<=n-gi&&(u=p,l={x:m,y:h,dx:g[1],dy:g[2]});break}}return l}var bi=[.958,.287];function xi(e,t){let n=e*bi[0]+t*bi[1],r=bi[0]*t-bi[1]*e,i=n<0?-1:1;return Math.max(0,Math.min(1,Math.atan2(r*i,n*i)/Math.PI+.5))}var Si=2.2;function Ci(e,t,n,r,i,a){let o=new Map;for(let[e,n,r,i]of t)for(let[t,a]of[[e,n],[r,i]]){let e=`${Math.round(t*20)},${Math.round(a*20)}`,n=o.get(e);n?n.n++:o.set(e,{x:t,y:a,n:1})}let s=new Set,c=[...o.values()].filter(e=>e.n!==2).map(e=>[e.x,e.y]);for(let t of e.furniture)t.kind===`outfall`&&c.push([t.x+t.w/2,t.y+t.h/2]);for(let[e,t]of c){let[a,o]=[Math.max(0,Math.floor(e-Si)),Math.max(0,Math.floor(t-Si))],[c,l]=[Math.min(n-1,Math.floor(e+Si)),Math.min(r-1,Math.floor(t+Si))];for(let r=o;r<=l;r++)for(let o=a;o<=c;o++){let a=r*n+o;if(i[a*4]!==3||i[a*4+3]===0)continue;let c=Math.round(255*Math.max(0,1-Math.hypot(o+.5-e,r+.5-t)/Si));i[a*4+2]=Math.max(c,s.has(a)?i[a*4+2]:0),s.add(a)}}for(let e=0;e<n*r;e++)(i[e*4+3]===0||i[e*4]===3)&&(i[e*4+2]=(s.has(e)?i[e*4+2]:0)&248|a&7)}function wi(e,t){let n=e.levels[t],{nx:r,ny:i}=e,a=n.paths??[],o=a.length?Zr:0,s=o*4,[c,l]=[(r+2*o)*4,(i+2*o)*4],u=new Float32Array(c*l),d=new Float32Array(c*l),f=new Float32Array(c*l),p=new Float32Array(c*l).fill(.5),m=new Uint8Array(r*i),h=0;for(let e of n.furniture)(e.kind===`sewage`||e.kind===`lava`)&&(m[e.y*r+e.x]=1,h=e.kind===`sewage`?1:2);let g=new Uint8Array(r*i*4),_=li(n,r,i),{index:v,shapes:y,caverns:b}=Hr(n,r,i),x=!1;for(let t=0;t<i;t++)for(let o=0;o<r;o++){let l=t*r+o,p=n.cells[l];if(p<0)continue;let h=n.rooms[p];g[l*4]=h.shape===255?0:_[l]?12:ci(e.function,h.kind,e.theme),g[l*4+1]=v[l],g[l*4+2]=p*37&255,g[l*4+3]=255;let y=(!a.length||h.kind!==`sewer tunnel`)&&Ur(v[l]),b=Math.min(1,Mt(n,r,i,o,t)/ii);b>0&&(x=!0);for(let e=t*4;e<(t+1)*4;e++){let t=(e+s)*c+s;for(let e=o*4;e<(o+1)*4;e++)y&&(u[t+e]=1),d[t+e]=m[l],f[t+e]=b}}if(a.length){let t=ui(n,r,i,o),s=r+2*o,f=new Float32Array(c*l),m=new Float32Array(c*l).fill(1/0);for(let[e,n,r,i,u]of a){let a=r-e,d=i-n,h=Math.max(a*a+d*d,1e-9),g=u*u,_=xi(a,d),v=Math.max(0,Math.floor((Math.min(e,r)-u+o)*4)),y=Math.max(0,Math.floor((Math.min(n,i)-u+o)*4)),b=Math.min(c-1,Math.ceil((Math.max(e,r)+u+o)*4)),x=Math.min(l-1,Math.ceil((Math.max(n,i)+u+o)*4));for(let r=y;r<=x;r++){let i=(r+.5)/4-o,l=(Math.floor(i)+o)*s+o;for(let s=v;s<=b;s++){let u=(s+.5)/4-o;if(!t[l+Math.floor(u)])continue;let v=Math.max(0,Math.min(1,((u-e)*a+(i-n)*d)/h)),y=u-e-v*a,b=i-n-v*d,x=y*y+b*b;x>g||(f[r*c+s]=1,x<m[r*c+s]&&(m[r*c+s]=x,p[r*c+s]=_))}}}h=1,d.set(vi(a,t,f,r,i,o,c,l));for(let e=0;e<c*l;e++)f[e]&&(u[e]=1);Ci(n,a,r,i,g,e.stone??0)}let S=oi.has(e.function)?u:Y(u,c,l,(n.natural?.42:a.length?.22:.14)*4),C=d;h===1&&(C=Y(d,c,l,1.2)),h===2&&(C=Y(d,c,l,2.4).map(e=>Math.min(1,e*1.7)));let w=x?Y(f,c,l,1.2):f,T=o?Qr:0,E=(o-T)*4,[D,O]=[c-2*E,l-2*E],k=new Uint8Array(D*O*4);for(let e=0;e<O;e++)for(let t=0;t<D;t++){let n=(e+E)*c+t+E,r=(e*D+t)*4;k[r]=S[n]*255+.5|0,k[r+1]=C[n]*255+.5|0,k[r+2]=w[n]*255+.5|0,k[r+3]=p[n]*255+.5|0}for(let e=0;e<r*i;e++)g[e*4+1]=v[e];let A=Math.max(1,y.length),j=new Uint8Array(A*2*4);y.forEach(({box:t,shape:r,room:i},a)=>{j.set([t[0],t[1],t[2],t[3]],a*4),j.set([r,ci(e.function,n.rooms[i].kind,e.theme),i*37&255,255],(A+a)*4)});let M=null;if(b){let[e,t]=[r*4,i*4],a=new Float32Array(e*t);for(let t=0;t<i;t++)for(let o=0;o<r;o++){let s=t*r+o;if(n.cells[s]<0)continue;let c=(e,t)=>e>=0&&t>=0&&e<r&&t<i&&v[t*r+e]===255;if(v[s]===255||c(o-1,t)||c(o+1,t)||c(o,t-1)||c(o,t+1))for(let n=t*4;n<(t+1)*4;n++)a.fill(1,n*e+o*4,n*e+(o+1)*4)}let o=Y(a,e,t,1.68);M=new Uint8Array(e*t*4);for(let n=0;n<e*t;n++)M[n*4]=o[n]*255+.5|0}let ee=Sn(S,c,l,4,o,r,i,(e,t)=>n.cells[t*r+e]>=0&&!Ur(v[t*r+e]));return{w:D,h:O,pad:T,data:k,cells:g,liquidKind:h,shapes:j,shapeCount:y.length,cave:M,terrain:Ei(e,t),dist:ee,lift:Ti(e,t)}}function Ti(e,t){let n=e.levels[t],{nx:r,ny:i}=e;if(n.paths?.length)return null;let a=Et(n,r,i);if(!a)return null;let{v:o,h:s}=jt(a,n),c=new Uint8Array(r*i);for(let e of n.furniture)if(e.kind===`rubble_slope`)for(let t=e.y;t<Math.min(i,e.y+e.h);t++)for(let n=e.x;n<Math.min(r,e.x+e.w);n++)c[t*r+n]=1;let l=e=>a.ramp[e]&&c[e]===1,u=[];for(let e=0;e<i;e++)for(let t=0;t<=r;t++)o[e*(r+1)+t]&&u.push([t,e,1,o[e*(r+1)+t],a.ft[e*r+t-1]]);for(let e=0;e<=i;e++)for(let t=0;t<r;t++)s[e*r+t]&&u.push([t,e,0,s[e*r+t],a.ft[(e-1)*r+t]]);let d=oi.has(e.function),f=d?8:4,[p,m]=[r*f,i*f];if(!u.length&&!a.ramp.some(e=>e))return null;let h=new Float32Array(p*m).fill(1/0),g=new Float32Array(p*m),_=new Float32Array(p*m);for(let[e,t,o,s,c]of u){let[l,u]=[e-J,t-J],[v,y]=o?[e+J,t+1+J]:[e+1+J,t+J];for(let b=Math.max(0,Math.floor(u*f));b<Math.min(m,Math.ceil(y*f));b++)for(let u=Math.max(0,Math.floor(l*f));u<Math.min(p,Math.ceil(v*f));u++){let[l,m]=[(u+.5)/f,(b+.5)/f],[v,y,x]=o?[l-e,m,t]:[m-t,l,e],S=Math.max(0,x-y,y-x-1),C=d?Math.max(Math.abs(v),S):Math.hypot(v,S),w=C+.001*S,T=b*p+u;if(C>=J||w>=h[T])continue;h[T]=w;let E=Math.min(i-1,Math.floor(m))*r+Math.min(r-1,Math.floor(l)),D=v>0==s>0;n.cells[E]>=0&&!a.ramp[E]&&(a.ft[E]>=Math.max(c,c+s)?D=!0:a.ft[E]<=Math.min(c,c+s)&&(D=!1)),g[T]=D?C:-C,_[T]=Math.abs(s)}}let v=new Int8Array(r*i),y=[];for(let e=0;e<r*i;e++){let t=(Math.floor(e/r)*f+(f>>1))*p+e%r*f+(f>>1);h[t]<1/0&&(v[e]=g[t]>0?1:-1,y.push(e))}for(let e of[!0,!1]){let t=e?y:Array.from(v.keys()).filter(e=>v[e]);for(let o=0;o<t.length;o++){let s=t[o],[c,l]=[s%r,Math.floor(s/r)];for(let o of[c>0?s-1:-1,c+1<r?s+1:-1,l>0?s-r:-1,l+1<i?s+r:-1])o<0||v[o]||(e?n.cells[s]>=0&&n.cells[o]>=0&&a.ft[o]===a.ft[s]:n.cells[o]<0)&&(v[o]=v[s],t.push(o))}}for(let e=0;e<p*m;e++)h[e]===1/0&&(g[e]=(v[Math.floor(Math.floor(e/p)/f)*r+Math.floor(e%p/f)]||-1)*J);let b=new Float32Array(p*m).fill(1);for(let e=0;e<i;e++)for(let t=0;t<r;t++)if(a.ramp[e*r+t])for(let n=Math.max(0,Math.floor((e-ri)*f));n<Math.min(m,Math.ceil((e+1+ri)*f));n++)for(let r=Math.max(0,Math.floor((t-ri)*f));r<Math.min(p,Math.ceil((t+1+ri)*f));r++){let[i,a]=[(r+.5)/f,(n+.5)/f],o=Math.hypot(Math.max(0,t-i,i-t-1),Math.max(0,e-a,a-e-1)),s=n*p+r;b[s]=Math.min(b[s],Math.min(1,o/ri))}let x=(e,t,o)=>{let s=[];for(let[c,l]of[[-1,0],[1,0],[0,-1],[0,1]]){let[u,d]=[e+c,t+l];if(u<0||d<0||u>=r||d>=i)continue;let f=d*r+u;if(n.cells[f]<0||a.ramp[f]||a.ft[f]===a.ft[o])continue;let[p,m]=[c>0?e+1:e,l>0?t+1:t];s.push([p,m,c?p:p+1,l?m:m+1,a.ft[f]])}return s},S=[];for(let e=0;e<r*i;e++)if(l(e))for(let t of x(e%r,Math.floor(e/r),e))S.push([...t,a.ft[e]]);let C=new Float32Array(p*m).fill(-1),w=new Uint8Array(p*m),T=new Uint8Array(p*m),E=new Uint8Array(r*i),D=new Uint8Array(r*i);for(let e=0;e<r*i;e++){if(!a.ramp[e]||D[e])continue;let t=[e];D[e]=1;let[o,s,c,l]=[0,0,1/0,-1/0];for(let e=0;e<t.length;e++){let u=t[e],[d,f]=[u%r,Math.floor(u/r)];for(let[e,p]of[[-1,0],[1,0],[0,-1],[0,1]]){let[m,h]=[d+e,f+p];if(m<0||h<0||m>=r||h>=i)continue;let g=h*r+m;if(n.cells[g]<0)continue;if(a.ramp[g]){D[g]||(D[g]=1,t.push(g));continue}let _=Math.sign(a.ft[g]-a.ft[u]);[o,s]=[o+_*e,s+_*p],_<0&&(c=Math.min(c,a.ft[g])),_>0&&(l=Math.max(l,a.ft[g]))}}let u=(Math.round(Math.atan2(s,o)/(Math.PI/4))+8)%8,d=c<1/0&&l>-1/0?l-c:c<1/0||l>-1/0?2*Math.abs(a.ft[e]-(c<1/0?c:l)):0,f=Math.min(248,Math.round(d/ii*255/8)*8)+u;for(let e of t)E[e]=f}if(S.length){let e=(e,t,n)=>Math.hypot(e-Math.min(Math.max(e,n[0]),n[2]),t-Math.min(Math.max(t,n[1]),n[3]));for(let t=0;t<i;t++)for(let n=0;n<r;n++){if(!l(t*r+n))continue;let i=S.filter(e=>e[0]>=n-3&&e[2]<=n+4&&e[1]>=t-3&&e[3]<=t+4);for(let a=Math.max(0,t*f-1);a<Math.min(m,(t+1)*f+1);a++)for(let o=Math.max(0,n*f-1);o<Math.min(p,(n+1)*f+1);o++){let[s,c]=[(o+.5)/f,(a+.5)/f],[l,u]=[1/0,1/0];for(let t of i){let n=e(s,c,t);t[4]<t[5]&&(l=Math.min(l,n)),t[4]>t[5]&&(u=Math.min(u,n))}if(l===1/0&&u===1/0)continue;let d=a*p+o,m=o>=n*f&&o<(n+1)*f&&a>=t*f&&a<(t+1)*f;(m||!T[d])&&(m&&(T[d]=1),C[d]=u===1/0?1:l===1/0?0:l/(l+u),m&&(w[d]=E[t*r+n]))}}}let[O,k]=[g,_];if(!d){let e=.36*f;[O,k]=[Y(g,p,m,e),Y(_,p,m,e)]}let A=new Uint8Array(p*m*4);for(let e=0;e<p*m;e++)A[e*4]=Math.max(1,Math.min(255,Math.round(128+O[e]*ni))),A[e*4+1]=Math.min(255,Math.round(k[e]/ii*255)),A[e*4+2]=Math.round(b[e]*255),C[e]>=0&&(A[e*4+3]=1+Math.round(C[e]*254)),T[e]&&(A[e*4+1]=w[e]);return{data:A,res:f}}function Ei(e,t){let n=e.levels[t],{nx:r,ny:i}=e,a=n.furniture.filter(e=>ai[e.kind]!==void 0);if(!a.length)return null;let o=new Uint8Array(r*i*4);for(let e of a){let t=ai[e.kind];for(let n=Math.max(0,e.y);n<Math.min(i,e.y+e.h);n++)for(let i=Math.max(0,e.x);i<Math.min(r,e.x+e.w);i++)o[(n*r+i)*4+t]=1,t===1&&(o[(n*r+i)*4]=1)}let s=o.slice();for(let e=0;e<i;e++)for(let t=0;t<r;t++)if(!(n.cells[e*r+t]>=0)){for(let a=Math.max(0,e-1);a<=Math.min(i-1,e+1);a++)for(let i=Math.max(0,t-1);i<=Math.min(r-1,t+1);i++)if(!(n.cells[a*r+i]<0))for(let n=0;n<4;n++)s[(e*r+t)*4+n]|=o[(a*r+i)*4+n]}let[c,l]=[r*4,i*4],u=new Uint8Array(c*l*4);for(let e=0;e<4;e++){if(!a.some(t=>ai[t.kind]===e||e===0&&t.kind===`water_deep`))continue;let t=new Float32Array(c*l);for(let n=0;n<i;n++)for(let i=0;i<r;i++)if(s[(n*r+i)*4+e])for(let e=n*4;e<(n+1)*4;e++)t.fill(1,e*c+i*4,e*c+(i+1)*4);let n=Y(t,c,l,(e===2?.24:.3)*4);for(let t=0;t<c*l;t++)u[t*4+e]=n[t]*255+.5|0}return u}function Di(e,t){let n=dn(),[r,i]=[Math.floor(e),Math.floor(t)],[a,o]=[e-r,t-i];a=a*a*(3-2*a),o=o*o*(3-2*o);let s=(e,t)=>n[((t&511)*512+(e&511))*4];return((s(r,i)*(1-a)+s(r+1,i)*a)*(1-o)+(s(r,i+1)*(1-a)+s(r+1,i+1)*a)*o)/255}var Oi=(e,t)=>.55*Di(e,t)+.3*Di(e*2.13+7.1,t*2.13+7.1)+.15*Di(e*4.37+3.3,t*4.37+3.3);function ki(e,t,n,r){let{w:i,h:a,pad:o,data:s}=e,c=(e,t)=>{let n=Math.max(0,Math.min(i-1,(e+o)*4-.5)),r=Math.max(0,Math.min(a-1,(t+o)*4-.5)),[c,l]=[Math.floor(n),Math.floor(r)],[u,d]=[n-c,r-l],[f,p]=[Math.min(i-1,c+1)-c,(Math.min(a-1,l+1)-l)*i],m=(l*i+c)*4;return((s[m]*(1-u)+s[m+f*4]*u)*(1-d)+(s[m+p*4]*(1-u)+s[m+(p+f)*4]*u)*d)/255},l=.25,u=(e,t)=>{let[n,i]=[(c(e+l,t)-c(e-l,t))/(2*l),(c(e,t+l)-c(e,t-l))/(2*l)],a=(c(e,t)-.5)/Math.max(Math.hypot(n,i),.35);return r?a+r*((Oi(e*.8+3,t*.8+3)-.5)*.75+(Di(e*3.7,t*3.7)-.5)*.25+(Di(e*9,t*9)-.5)*.08):a},d=new Uint8Array(t*n);for(let r=0;r<t*n;r++)d[r]=e.cells[r*4+1];return Object.assign(u,{shaped:d})}var Ai=24,ji=.25,Mi=.3;function Ni(e,t,n){let[r,i]=[e(t-Mi,n)-e(t+Mi,n),e(t,n-Mi)-e(t,n+Mi)],a=Math.max(Math.hypot(r,i),1e-9);return[r/a,i/a]}function Pi(e,t,n,r,i,a){let o=e(t,n),s=[],c=r;for(let r=0;r<Ai;r++){let[a,l]=[Math.cos(r/Ai*Math.PI*2),Math.sin(r/Ai*Math.PI*2)],u=o;for(let r=ji;r<=c+ji/2;r+=ji){let d=e(t+a*r,n+l*r);if(d>=0!=o>=0){let e=r-ji+ji*u/(u-d);s.push({t:e,x:t+a*e,y:n+l*e,ux:a,uy:l}),c=Math.min(c,e+i);break}u=d}}s.sort((e,t)=>e.t-t.t);let l=null,u=2;for(let e of s){if(e.t>s[0].t+i)break;let t=a(e.x,e.y,e.ux,e.uy);if(t<u&&([l,u]=[e,t]),!t)break}if(!l)return null;let{x:d,y:f}=l,[p,m,h]=[d,f,Math.abs(e(d,f))];for(let t=0;t<3&&h<=Mi;t++){let t=e(p,m),[n,r]=Ni(e,p,m);p+=n*t,m+=r*t;let i=Math.abs(e(p,m));i<h&&([d,f,h]=[p,m,i])}let[g,_]=Ni(e,d,f);return{x:d,y:f,dx:g,dy:_}}var Fi=`#version 300 es
// Underground levels, painterly (the battlemap's style): smooth floor outlines from a blurred
// floor field (rock walls roughened by noise in caves and mines, near-straight dressed stone in
// built sites), a lit inked rim where the wall meets the floor, deep textured rock beyond,
// ambient occlusion and NW-light shadows on the floor, floors by material (cave rock, basalt,
// mine earth, sewer brick, flagstones), animated sewage or lava channels, ledges and steps (an
// inked lip, a hatched cliff face, a shadow below), terrain (water with slow ripples, chasms, rubble slopes), and the
// 5-ft grid.
//
// uField: 4 texels per square, linear: r floor (blurred mask), g liquid (blurred), b raised
//         floor (blurred, raise / 20 ft), a the way sewage runs (\`flowAngle\` in underField.ts). Built sites (uSquare): nearest, the floor unblurred;
//         their walls are measured from the squares instead, keeping square corners.
// uCells: one texel per square, nearest: r material, g shape (0 none, a shaped room's slot + 1
//         in uShapes, 255 a cavern breaking in), b variation (a sewer's brick: foam), a floor.
// uShapes: shaped rooms (round, octagon, apses, chamfered: \`roomShapes.ts\`), one column each:
//         row 0 the box round its squares (x0, y0, x1, y1), row 1 shape, material, variation.
//         Their squares are left out of uField and the squares' walls; the floor is drawn from
//         the shape near them (it may stand up to half a square off the squares).
// uCave: caverns breaking into a built level, linear: their floor (and the squares opening into
//         them) blurred as a cave's, drawn with rough rock walls.
// uTerrain: terrain (\`kit::TERRAIN\`), linear, 4 texels per square: r water (shallow, and under
//         deep water), g deep water, b chasm, a rubble slope; each blurred, spread into the rock
//         beside it (so it meets the walls). Read only where uHasTerrain.
// uLift: ledges and steps (\`liftField\` in underField.ts), linear, 8 texels a square in built sites
//         (else 4, blurred as the walls): r the signed distance to the nearest edge between floor
//         heights (squares, + on its high side, 128 + 63.5 d), g its rise (/ 20 ft), b how strongly it
//         is drawn (0 on ramps, tapering up within 0.4 sq of one), a on and by ramps 1 + 254 t (t how
//         far up the ramp, 0 at its low floor; its rise in g there; 0 elsewhere). Read only where uHasLift.
// uDist: squares to the nearest floor (\`rockDistance.ts\`), linear, 4 texels per square, DIST_PAD
//         squares past the grid each way, DIST_MAX at 1: the rock is hatched beside the walls by
//         it and fades out beyond (one fetch; the walls themselves come from the fields above).

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform highp sampler2D uNoise;
uniform highp sampler2D uField;
uniform highp sampler2D uCells;
uniform highp sampler2D uShapes;
uniform highp sampler2D uCave;
uniform highp sampler2D uTerrain;
uniform highp sampler2D uLift;
uniform highp sampler2D uDist;

uniform vec2 uN;
uniform vec2 uLight;
uniform float uTime;
uniform float uRough;
uniform float uSquare;
uniform float uLiquid;
uniform float uGhost;
uniform float uGridAlpha;
uniform float uGrid;
// Where this grid sits among its neighbours (squares; a city's sewer sections, so their
// textures run on across the edges), else 0.
uniform vec2 uOrigin;
// Squares of uField beyond the grid each way (a sewer section's, read past its edge so that its
// walls and channels run on), else 0.
uniform float uPad;
// Shaped rooms on the level (0: none), caverns breaking in (0 or 1).
uniform float uShapeCount;
uniform float uCaverns;
uniform float uHasTerrain;
uniform float uHasLift;
// Squares the quad is drawn past the grid each way (a site's, so its rock fades out round it;
// sewer sections meet edge to edge: 0).
uniform float uGrow;
uniform vec4 uColor;
// The site's stone (\`underTheme.ts\`): its wall style (0 plain, 1 dwarven rune-cut blocks with
// thick walls, 2 rough ashlar, 3 dark basalt, 4 pale dusty stone, 5 wet brick, 6 marble, 7 packed
// earth, 8 natural rock: one branch for the whole site), flagstones, the coping on built walls
// and the hatched rock's tone.
uniform float uTheme;
uniform vec3 uStone;
uniform vec3 uCoping;
uniform vec3 uRock;
// A cave's look (\`underTheme.ts\`): 0 none, 1 green, 2 dry, 3 cold, 4 ice; and its cast on the floors.
uniform float uLook;
uniform vec3 uTint;

const vec3 INK = vec3(0.10, 0.085, 0.07);

// Squares a pixel (set first thing in \`main\`, for the functions below).
float gPx;
// How much of this pixel is a ramp (\`ledges\`): the rubble slope's dust and stones thin out over it.
float gRamp = 0.0;
// A built site's raised floors: the dressed stone their faces are cut from, in one flat tone (the
// floor's own flags would carry their joints and light and dark stones down the face).
vec3 gFace = vec3(0.5);
// How much of this pixel is a built ledge's face or its shadow (\`ledgeEdge\`): there the wall's shade
// is lifted as on the gallery's top (else a gallery by the wall has its face in the wall's cast
// shadow, near black, and the drop reads as a trough).
float gLedge = 0.0;
// How far \`d\` (squares to the wall) is a true distance here, 0-1: the floor field's slope is too flat for it
// out in a wide floor's middle.
float gTrue = 1.0;
// How much of this pixel \`terrain\` covers (water, a chasm, a slope): 0 where it has none.
float gTerr = 0.0;

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return texture(uNoise, (i + f + 0.5) / 512.0).r;
}

float fbm(vec2 p) {
    return 0.55 * vnoise(p) + 0.3 * vnoise(p * 2.13 + 7.1) + 0.15 * vnoise(p * 4.37 + 3.3);
}

vec4 field(vec2 q) {
    vec2 u = (q + uPad) / (uN + 2.0 * uPad);
    // (Past the texture, round a site's grid: rock.)
    if (any(lessThan(u, vec2(0.0))) || any(greaterThan(u, vec2(1.0)))) return vec4(0.0, 0.0, 0.0, 0.5);
    return texture(uField, u);
}

// Thin dark seams where noise crosses a level (cracks in rock and stone): about \`w\` of the
// noise's range wide, never wider than ~1.5 px (so close up they stay ink lines, not bands).
float seam(float v, float w) {
    float px = fwidth(v) * 1.5;
    return 1.0 - smoothstep(0.0, max(min(w, px), 1e-4), abs(v - 0.5));
}

// (\`p\`: position for noise, continuous from one section of sewers to the next.)
vec3 rockMass(vec2 p, float depth) {
    vec3 c = vec3(0.25, 0.23, 0.21);
    c *= 0.72 + 0.5 * vnoise(p * 0.55) + 0.12 * (vnoise(p * 2.4 + 5.0) - 0.5);
    // Fractures.
    c = mix(c, c * 0.6, seam(vnoise(p * 1.1 + 3.0), 0.03));
    // Deeper into the rock is darker.
    return c * mix(1.0, 0.5, smoothstep(0.0, 2.5, depth));
}

const float DIST_PAD = 4.0;
const float DIST_MAX = 8.0;

// The rock beside the walls, inked as on a hand-drawn dungeon map: short parallel strokes in
// small clusters, each cluster at its own angle, densest at the wall and thinning out, on a
// stone tone. \`depth\`: squares into the rock; \`px\`: squares a pixel. Strokes under ~2 px apart
// give way to their average tone (no shimmer). Clusters are the cells of a jittered grid
// (nearest centre), from hashes alone: one noise lookup, for the tone.
vec3 hatchedRock(vec2 p, float depth, float px, out float cut, out float ra) {
    cut = 0.0;
    // (Ice rock flat: one pale tone, no mottling.)
    vec3 c = uLook > 3.5 ? uRock : uRock * (0.92 + 0.16 * vnoise(p * 0.55));
    // (Ice rock is inked in a deep blue-grey, not black.)
    vec3 inkc = uLook > 3.5 ? vec3(0.36, 0.48, 0.6) : mix(INK, uRock, 0.18);
    // (The flat tone only close to the wall, ragged per cluster, so the rock is never paler than the ground.)
    float toneA = 0.55;
    float inkA = 0.0;
    float out1 = smoothstep(0.3, 2.0, depth);
    // Strokes this far apart (squares), half this thick (squares: about a pixel, bolder close up).
    const float GAP = 0.20;
    float hw = min(max(mix(0.03, 0.022, out1), 0.55 * px), 0.3 * GAP);
    float sharp = smoothstep(1.8, 3.0, GAP / max(px, 1e-5));
    float dens = mix(0.95, 0.15, out1);
    if (sharp > 0.0) {
        vec2 hc = p * 1.1;
        vec2 base = floor(hc);
        float best = 9.0;
        float second = 9.0;
        vec2 id = base;
        vec2 rel = vec2(0.0);
        for (int j = -1; j <= 1; j++) {
            for (int i = -1; i <= 1; i++) {
                vec2 cc = base + vec2(float(i), float(j));
                vec2 dv = hc - cc - 0.15 - 0.7 * vec2(hash21(cc), hash21(cc + 17.3));
                float dd = dot(dv, dv);
                if (dd < best) {
                    second = best;
                    best = dd;
                    id = cc;
                    rel = dv;
                } else if (dd < second) {
                    second = dd;
                }
            }
        }
        // (A thin gap between clusters; whole clusters drop out farther from the wall.)
        float edge = (sqrt(second) - sqrt(best)) / 1.1;
        // (Each cluster in or out as a whole; the band ends in ink, not in a haze.)
        float keep = step(hash21(id + 9.1), dens) * step(hash21(id + 11.9), 1.0 - smoothstep(1.2, 3.0, depth));
        // (Straight, parallel strokes: one angle for the whole cluster. The cluster's own
        // shift of where the tone fades out breaks up the outer edge.)
        float a = 3.14159265 * hash21(id + 3.7);
        cut = (hash21(id + 4.9) - 0.5) * 0.8;
        toneA = 0.55 * (1.0 - smoothstep(0.8, 1.6, depth + cut));
        vec2 dir = vec2(cos(a), sin(a));
        vec2 r = rel / 1.1;
        float t = dot(r, vec2(-dir.y, dir.x)) / GAP + hash21(id + 5.3);
        float row = floor(t);
        float h = hash21(vec2(row, 1.3) + id * 7.1);
        // Each stroke a little shorter or longer, a little off the cluster's middle.
        float u = dot(r, dir) - (h - 0.5) * 0.16;
        float half1 = (0.30 + 0.12 * fract(h * 13.7)) * mix(1.0, 0.6, out1);
        float aa = 0.8 * px;
        // (Width varies a quarter either way per stroke; ends round off.)
        float hws = hw * (0.75 + 0.5 * fract(h * 7.3));
        float dy = abs(fract(t) - 0.5) * GAP;
        float end = abs(u) - (half1 - hws);
        float shape = length(vec2(dy, max(end, 0.0)));
        float ink = 1.0 - smoothstep(hws - aa, hws + aa, shape);
        // (Clear of ink just by the wall's lip.)
        ink *= smoothstep(0.1, 0.15 + aa, depth);
        ink *= smoothstep(0.09, 0.12 + aa, edge) * keep;
        inkA = ink * 0.88 * sharp;
    } else {
        toneA = 0.55 * (1.0 - smoothstep(0.8, 1.6, depth));
    }
    // (Too fine to draw: their tone.)
    inkA = max(inkA, (1.0 - sharp) * 2.0 * hw / GAP * dens * 0.45);
    // (Ice: the pale tone solid over the whole rock, so the dark ground dimmed below never shows through
    // between the rooms as smoky dark stone.)
    if (uLook > 3.5) toneA = 0.88;
    ra = mix(toneA, max(toneA, 0.7), inkA);
    return mix(c, inkc, inkA);
}

// A square drawn from the squares: no shape on it (0, or 254: one next to it).
bool fromSquares(float g) {
    float c = g * 255.0;
    return c < 0.5 || abs(c - 254.0) < 0.5;
}

// Built sites: signed distance (squares, + on the floor) to the nearest square of the other
// kind (rock or floor; off the grid is rock), measured square-wise (the larger of the x and
// y gaps), so the wall, its coping and its shadow all keep square corners; and the way from
// the rock toward the floor. Exact within two squares (deeper rock reads as two).
void boxField(vec2 q, out float d, out vec2 n) {
    ivec2 s = clamp(ivec2(floor(q)), ivec2(0), ivec2(uN) - 1);
    vec4 own = texelFetch(uCells, s, 0);
    // (Off the grid, round a site: rock.)
    bool on = all(greaterThanEqual(q, vec2(0.0))) && all(lessThan(q, uN));
    bool fl = on && own.a > 0.5 && fromSquares(own.g);
    float best = 2.0;
    vec2 way = vec2(0.0);
    for (int j = -2; j <= 2; j++) {
        for (int i = -2; i <= 2; i++) {
            // (The outer ring is no nearer than one square: only needed when nothing is.)
            if (max(abs(i), abs(j)) == 2 && best < 1.0) continue;
            ivec2 c = s + ivec2(i, j);
            bool inside = all(greaterThanEqual(c, ivec2(0))) && all(lessThan(c, ivec2(uN)));
            // (A shaped room's squares are drawn from its shape.)
            vec4 t = inside ? texelFetch(uCells, c, 0) : vec4(0.0);
            bool f = t.a > 0.5 && fromSquares(t.g);
            if (f == fl) continue;
            vec2 lo = vec2(c);
            vec2 gap = max(max(lo - q, q - lo - 1.0), 0.0);
            float m = max(gap.x, gap.y);
            if (m < best) {
                best = m;
                way = q - clamp(q, lo, lo + 1.0);
            }
        }
    }
    d = fl ? best : -best;
    n = length(way) > 1e-5 ? normalize(fl ? way : -way) : vec2(0.0);
}

// Shapes: signed distance (squares, + inside) to a box, an ellipse (about its middle; close to
// the edge, as the walls need), an apse (its round end at low y), and to each shape in its box.
float boxIn(vec2 p, vec2 lo, vec2 hi) {
    vec2 d = min(p - lo, hi - p);
    if (d.x >= 0.0 && d.y >= 0.0) return min(d.x, d.y);
    return -length(max(-d, 0.0));
}

float ellipseIn(vec2 p, vec2 r) {
    float k0 = length(p / r);
    float k1 = length(p / (r * r));
    return k0 < 1e-4 ? min(r.x, r.y) : (1.0 - k0) * k0 / k1;
}

float apseIn(vec2 p, vec2 lo, vec2 hi) {
    vec2 sz = hi - lo;
    float r = min(sz.x * 0.5, sz.y - 1.0);
    if (r < 1.0) return boxIn(p, lo, hi);
    float rect = boxIn(p, vec2(lo.x, lo.y + r), hi);
    // (The whole ellipse, cut only by the box's far side: cut along its middle as well, the seam
    // where it meets the rectangle read as a wall, and drew a band of wall shade across the room.)
    float end = min(ellipseIn(p - vec2(lo.x + sz.x * 0.5, lo.y + r), vec2(sz.x * 0.5, r)), hi.y - p.y);
    return max(rect, end);
}

// 1 round, 2 octagon, 3-6 apse north, east, south, west, 7 chamfered (\`interior/shape.rs\`).
float shapeIn(int code, vec2 p, vec2 lo, vec2 hi) {
    vec2 c = 0.5 * (lo + hi);
    vec2 hs = 0.5 * (hi - lo);
    if (code == 1) {
        float r = min(hs.x, hs.y);
        vec2 e = abs(p - c) - (hs - r);
        return -(length(max(e, 0.0)) + min(max(e.x, e.y), 0.0) - r);
    }
    if (code == 2 || code == 7) {
        float side = 2.0 * min(hs.x, hs.y);
        float cut = code == 2 ? 0.29289322 * side : (side >= 3.0 ? 0.75 : 0.5);
        vec2 d = abs(p - c);
        return min(boxIn(p, lo, hi), (hs.x + hs.y - cut - d.x - d.y) * 0.70710678);
    }
    if (code == 3) return apseIn(p, lo, hi);
    if (code == 5) return apseIn(vec2(p.x, -p.y), vec2(lo.x, -hi.y), vec2(hi.x, -lo.y));
    if (code == 6) return apseIn(p.yx, lo.yx, hi.yx);
    if (code == 4) return apseIn(vec2(p.y, -p.x), vec2(lo.y, -hi.x), vec2(hi.y, -lo.x));
    return boxIn(p, lo, hi);
}

// Shaped room \`slot\` (1-based): its box and look (shape, material, variation).
void shapeSlot(int slot, out vec2 lo, out vec2 hi, out vec3 look) {
    vec4 b = floor(texelFetch(uShapes, ivec2(slot - 1, 0), 0) * 255.0 + 0.5);
    lo = b.xy;
    hi = b.zw;
    look = floor(texelFetch(uShapes, ivec2(slot - 1, 1), 0).rgb * 255.0 + 0.5);
}

float segment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}

// A rune cut into a dwarven block (0-1: how much groove): a stave and one or two twigs off it,
// which and where by \`h\`. \`g\`: from the block's middle (squares). Gone below ~2 px a stroke.
float rune(vec2 g, float h) {
    float sx = sign(fract(h * 5.3) - 0.5);
    float y1 = 0.25 - 0.3 * fract(h * 3.1);
    float y2 = -0.05 - 0.2 * fract(h * 9.7);
    float r = segment(g, vec2(0.0, -0.3), vec2(0.0, 0.3));
    r = min(r, segment(g, vec2(0.0, y1), vec2(0.2 * sx, y1 + (fract(h * 2.3) > 0.5 ? 0.2 : -0.2))));
    if (fract(h * 7.7) > 0.45) r = min(r, segment(g, vec2(0.0, y2), vec2(-0.2 * sx, y2 - 0.18)));
    float w = 0.03;
    float aa = max(gPx * 0.8, 0.004);
    return (1.0 - smoothstep(w - aa, w + aa, r)) * (1.0 - smoothstep(0.02, 0.035, gPx));
}

vec3 floorColor(int mat, vec2 q, vec2 p, float var) {
    vec3 c;
    if (mat == 0) {
        // Cave floor: mottled rock in broad patches, fine cracks, grit (relief and pebbles
        // are added for every natural floor).
        c = vec3(0.49, 0.45, 0.40) * (0.84 + 0.24 * vnoise(p * 0.35));
        c = mix(c, c * vec3(0.94, 0.96, 1.0), smoothstep(0.4, 0.7, vnoise(p * 0.25 + 20.0)) * 0.4);
        c = mix(c, c * 0.68, seam(vnoise(p * 0.9 + 4.0), 0.02));
        c = mix(c, c * 0.82, smoothstep(0.8, 0.92, vnoise(p * 6.3 + 9.0)) * 0.5);
    } else if (mat == 1) {
        // Basalt: dark, columnar cracks, glints.
        c = vec3(0.30, 0.28, 0.27) * (0.85 + 0.25 * fbm(p * 0.8));
        c = mix(c, c * 0.45, seam(vnoise(p * 1.9 + 2.0), 0.04));
        c += vec3(0.12) * smoothstep(0.93, 0.98, vnoise(p * 9.0));
    } else if (mat == 2) {
        // Mine floor: packed earth and gravel.
        c = vec3(0.47, 0.39, 0.30) * (0.82 + 0.3 * fbm(p * 0.6));
        c = mix(c, vec3(0.40, 0.38, 0.36), smoothstep(0.75, 0.88, vnoise(p * 6.0)) * 0.6);
        c = mix(c, c * 0.75, smoothstep(0.4, 0.6, vnoise(p * vec2(0.6, 3.0))) * 0.3);
    } else if (mat == 3) {
        // Sewer walkway by the city (\`under::sewer_stone\`, in a section's low bits of \`var\`):
        // running-bond brick, dressed flags or rough rubble, warm (buff, sandstone) or cool;
        // each piece one of three flat tones, mid-tone joints, the NW edge of flags and rubble
        // lit. Pieces are numbered from the world (one across a section's edge is one piece).
        float st = uPad > 0.5 ? mod(var + 0.5, 8.0) - 0.5 : 0.0;
        float warm = step(3.5, st);
        float kind = st - 4.0 * warm;
        vec2 sc = kind < 0.5 ? vec2(2.0, 4.0) : kind < 1.5 ? vec2(1.0, 1.5) : vec2(2.4, 2.8);
        vec2 b = q * sc;
        float row = floor(b.y);
        float shift = kind < 1.5 ? 0.5 * mod(row, 2.0) : hash21(vec2(row + mod(uOrigin.y * sc.y, 1024.0), 4.1));
        b.x += shift;
        vec2 id = mod(floor(b) + mod(uOrigin * sc, 1024.0), 1024.0);
        vec2 g = fract(b);
        float tone = floor(hash21(id) * 2.999) * 0.5;
        // (Square units from the piece's edge, and the way out to it.)
        vec2 eg = min(g, 1.0 - g) / sc;
        vec2 out2 = eg.x < eg.y ? vec2(sign(g.x - 0.5), 0.0) : vec2(0.0, sign(g.y - 0.5));
        float edge = min(eg.x, eg.y);
        float jw = 0.035;
        if (kind > 1.5) {
            // Rubble: rounded stones of a few sizes, jittered in their cells.
            vec2 v = (g - 0.5 - 0.12 * (vec2(hash21(id + 2.2), hash21(id + 5.9)) - 0.5)) / sc;
            vec2 hs = (0.5 - 0.06 - 0.05 * hash21(id + 8.1)) / sc;
            float rr = 0.12;
            vec2 a = abs(v) - hs + rr;
            edge = rr - length(max(a, 0.0)) - min(max(a.x, a.y), 0.0);
            out2 = normalize(v + 1e-4);
            jw = 0.03;
        }
        vec3 base = kind < 0.5 ? mix(vec3(0.52, 0.41, 0.34), vec3(0.62, 0.52, 0.38), warm)
                  : kind < 1.5 ? mix(vec3(0.56, 0.55, 0.52), vec3(0.63, 0.56, 0.44), warm)
                  : mix(vec3(0.54, 0.52, 0.48), vec3(0.62, 0.55, 0.44), warm);
        c = base * (kind < 0.5 ? 0.9 + 0.14 * tone : 0.87 + 0.2 * tone);
        // Patches (one block of 3 x 3 squares in a few): repaired in another tone, following the
        // pieces; cracks across others.
        vec2 pc = (floor(b) + 0.5 - vec2(shift, 0.0)) / sc;
        vec2 pw = floor((q + uOrigin) / 3.0);
        float ph = hash21(mod(pw, 1024.0) + 11.3);
        vec2 pm = pw * 3.0 - uOrigin + 1.5 + (vec2(hash21(pw + 3.3), hash21(pw + 6.1)) - 0.5) * 0.8;
        vec2 pd = abs(pc - pm) - vec2(0.5, 0.45) - 0.5 * vec2(hash21(pw + 9.4), hash21(pw + 1.8));
        if (ph > 0.78 && max(pd.x, pd.y) < 0.0) {
            c = mix(c, (kind < 0.5 ? vec3(0.63, 0.56, 0.47) : vec3(0.49, 0.46, 0.42)) * (0.92 + 0.12 * tone), 0.85);
        }
        if (edge > 0.0) {
            // The edge toward the light lit, the far edge shaded (flags and rubble).
            float bev = (1.0 - smoothstep(0.02, 0.08, edge)) * step(0.5, kind);
            c *= 1.0 - 0.1 * bev * dot(out2, uLight) / max(length(uLight), 1e-4);
        }
        if (ph > 0.62 && ph <= 0.78) {
            float ca = 6.2832 * hash21(pw + 4.7);
            vec2 ct = vec2(cos(ca), sin(ca));
            vec2 cv = q - pm;
            float along = dot(cv, ct);
            float cd = dot(cv, vec2(-ct.y, ct.x)) + 0.12 * sin(along * 4.0 + ca) + 0.06 * sin(along * 9.0);
            float cw = 0.018 + fwidth(cd);
            c = mix(c, c * 0.68, (1.0 - smoothstep(cw * 0.5, cw, abs(cd))) * (1.0 - smoothstep(0.8, 1.3, abs(along))));
        }
        float joint = 1.0 - smoothstep(jw * 0.5, jw, edge);
        c = mix(c, mix(base * 0.6, vec3(0.34, 0.32, 0.28), 0.5), joint * (kind < 0.5 ? 0.85 : 0.6));
        c = mix(c, c * vec3(0.72, 0.77, 0.62), smoothstep(0.5, 0.8, fbm(p * 0.5 + 2.0)) * 0.4);
    } else if (mat == 7) {
        // Marble: a square slab per square, light and dark in turn, veined.
        vec2 cell = floor(q);
        vec2 g = fract(q);
        c = mix(vec3(0.80, 0.78, 0.74), vec3(0.60, 0.58, 0.56), mod(cell.x + cell.y, 2.0)) * (0.94 + 0.08 * hash21(cell));
        c = mix(c, c * 0.72, seam(fbm(p * 0.9 + cell * 0.37), 0.015));
        float joint = 1.0 - smoothstep(0.0, 0.02, min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y)));
        c = mix(c, INK * 1.8, joint * 0.3);
    } else if (mat == 8) {
        // Hewn rock: big blocks two squares long in running bond, tight joints. A dwarven hall's
        // two squares each way, a rune cut in some.
        bool dw = uTheme > 0.5 && uTheme < 1.5;
        vec2 b = q * (dw ? vec2(0.5) : vec2(0.5, 1.0));
        b.x += 0.5 * mod(floor(b.y), 2.0);
        vec2 g = fract(b);
        c = uStone * 0.9 * (0.85 + 0.2 * hash21(floor(b))) * (0.92 + 0.12 * vnoise(p * 2.0));
        float joint = 1.0 - smoothstep(0.0, dw ? 0.06 : 0.025, min(min(g.x, 1.0 - g.x) * 2.0, min(g.y, 1.0 - g.y) * (dw ? 2.0 : 1.0)));
        c = mix(c, INK * 1.5, joint * (dw ? 0.75 : 0.4));
        if (dw && hash21(floor(b) + 2.1) > 0.6) c = mix(c, c * 0.6, rune((g - 0.5) * 2.0 * vec2(1.0, 0.8), hash21(floor(b) + 6.3)));
    } else if (mat == 9) {
        // Ice: pale blue, cracked, glinting.
        c = vec3(0.74, 0.84, 0.90) * (0.88 + 0.16 * fbm(p * 0.5));
        c = mix(c, vec3(0.50, 0.64, 0.78), seam(vnoise(p * 1.3 + 2.0), 0.02));
        c += vec3(0.15) * smoothstep(0.94, 0.99, vnoise(p * 8.0));
    } else if (mat == 12) {
        // A fighting pit's packed sand (a boss chamber's sunken arena): pale earthy tan, paler and
        // yellower than the stone round it, in a few big soft shapes: raked paler here, trodden a
        // little darker and browner there (three close flat tones, no grain).
        c = vec3(0.63, 0.57, 0.46);
        c = mix(c, vec3(0.56, 0.49, 0.39), smoothstep(0.5, 0.66, vnoise(p * 0.32 + 5.0)) * 0.9);
        c = mix(c, vec3(0.69, 0.63, 0.52), smoothstep(0.6, 0.72, vnoise(p * 0.45 + 17.0)) * 0.7);
    } else {
        // Flagstones laid to the squares: one stone a square, or a square split in two along its
        // half line (which way and whether, by square). Boss chambers darker, crypts dusty.
        // (\`scale\` cancels out, but keep it: without it Adreno's compiler segfaults on this shader,
        // which kills every WebGL context on the page. Simpler ways of tying this to \`mat\` don't help.)
        float scale = mat == 5 ? 0.7 : mat == 10 ? 0.85 : 1.0;
        vec2 fq = q * scale / scale;
        vec2 sq = floor(fq);
        vec2 g = fract(fq);
        float h = hash21(sq + 3.7);
        vec2 stoneId = sq;
        vec2 e = vec2(0.5);
        if (h > 0.4) {
            // Split across x (two stones side by side) or across y (one above the other).
            bool alongX = h < 0.7;
            float side = step(0.5, alongX ? g.x : g.y);
            stoneId = sq + (alongX ? vec2(side * 0.5 + 0.25, 0.0) : vec2(0.0, side * 0.5 + 0.25));
            e = alongX ? vec2(abs(g.x - 0.5), 1.0) : vec2(1.0, abs(g.y - 0.5));
        }
        float edge = min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y));
        if (h > 0.4) edge = min(edge, min(e.x, e.y));
        edge += 0.025 * (vnoise(p * 6.0) - 0.5);
        float mortar = 1.0 - smoothstep(0.012, 0.04, edge);
        vec3 stone = mat == 5 ? uStone * vec3(0.76, 0.71, 0.74) : mat == 10 ? vec3(0.30, 0.28, 0.30) : uStone;
        c = stone * (0.8 + 0.32 * hash21(stoneId * 1.7 + var)) * (0.9 + 0.15 * vnoise(p * 3.0));
        c = mix(c, INK * 1.6, mortar * 0.35);
    }
    return c;
}

// A cave's biome on its floor: a few big flat patches (moss, drifted sand, frost); the green look's roots
// are \`caveRoots\`. \`d\`: squares from the wall, \`n\`: the wall's normal.
vec3 cave_look(vec3 col, vec2 p, float d, vec2 n) {
    if (uLook < 1.5) {
        col = mix(col, vec3(0.40, 0.50, 0.30), smoothstep(0.58, 0.62, vnoise(p * 0.2 + 31.0)) * 0.7);
    } else if (uLook < 2.5) {
        // (The noise turned off the squares and its lattice bent, so a drift's edge never runs straight.)
        vec2 dq = mat2(0.8, 0.6, -0.6, 0.8) * p * 0.17 + 0.32 * vec2(sin(p.y * 0.61 + 1.3), sin(p.x * 0.53 + 4.1));
        float drift = vnoise(dq + 7.0) + 0.3 * (1.0 - smoothstep(0.0, 1.6, d));
        col = mix(col, vec3(0.82, 0.72, 0.52), smoothstep(0.655, 0.705, drift) * 0.72);
    } else {
        float frost = vnoise(p * 0.19 + 13.0) + 0.3 * (1.0 - smoothstep(0.0, 1.4, d));
        col = mix(col, vec3(0.8, 0.87, 0.92), smoothstep(0.64, 0.68, frost) * 0.7);
    }
    return col;
}

// A tapered stroke from \`a\` (half width \`ra\`) to \`b\` (\`rb\`): signed distance, squares.
float taperSeg(vec2 p, vec2 a, vec2 b, float ra, float rb) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - mix(ra, rb, h);
}

// The green look's roots: a few a room, crawling out of the wall and along the floor's edge as the sprites
// draw them: a main root 1.8-2.8 squares long in three slightly crooked strokes, slanting off along the
// wall and tapering to a point, one side root reaching out over the floor, one diving back into the rock
// and a short one; flat mid brown inside a thin ink line (the sprites' \`LINE_W\`), a little wavy. One a cell
// (5 squares) at most, rooted on the wall by the cell's point (so each is whole or not there at all); the
// cells round are looked at too. Measured in the wall's frame here: \`d\` out from it (true only within
// a square or so of a rough wall, where the field still slopes: so they keep to that band), along it
// from the foot of the cell's point.
vec3 caveRoots(vec3 col, vec2 p, float d, vec2 n, float px) {
    // (Only by a wall: away from one the field is flat, \`n\` none and \`d\` no longer a distance.)
    if (d < 0.0 || d > 2.3 || gTrue < 0.01 || dot(n, n) < 0.5) return col;
    vec2 t = vec2(-n.y, n.x);
    float aa = max(px * 0.7, 1e-4);
    float lw = max(0.025, 0.9 * px);
    float dist = 9.0;
    vec2 base = floor(p / 5.0);
    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 cc = base + vec2(float(i), float(j));
            float h = hash21(cc + 61.0);
            if (h < 0.45) continue;
            vec2 c = (cc + 0.2 + 0.6 * vec2(hash21(cc + 7.0), hash21(cc + 13.0))) * 5.0;
            vec2 r = p - c;
            // (The cell's point near the wall here, else no root from it.)
            if (abs(dot(r, n) - d) > 1.8) continue;
            vec2 w = vec2(dot(r, t), d);
            float len = 1.8 + 1.0 * hash21(cc + 19.0);
            if (length(w) > len + 0.6) continue;
            // (A little wavy, as drawn by hand.)
            w += 0.045 * vec2(sin(w.y * 4.1 + h * 23.0), sin(w.x * 3.3 + h * 17.0));
            float dir = hash21(cc + 31.0) < 0.5 ? -1.0 : 1.0;
            float seg = len / 3.0;
            float a0 = dir * (0.8 + 0.3 * hash21(cc + 23.0));
            float a1 = clamp(a0 + dir * 0.15 + 0.4 * (hash21(cc + 29.0) - 0.5), -1.45, 1.45);
            float a2 = clamp(a1 + dir * 0.15 + 0.4 * (hash21(cc + 37.0) - 0.5), -1.5, 1.5);
            vec2 p0 = vec2(0.0, -0.15);
            vec2 p1 = p0 + seg * vec2(sin(a0), cos(a0));
            vec2 p2 = p1 + seg * vec2(sin(a1), cos(a1));
            vec2 p3 = p2 + seg * vec2(sin(a2), cos(a2));
            float dr = min(taperSeg(w, p0, p1, 0.2, 0.14), min(taperSeg(w, p1, p2, 0.14, 0.085), taperSeg(w, p2, p3, 0.085, 0.015)));
            // Side roots: out over the floor from the first joint, back into the rock from the second, and
            // a short one off the first stroke.
            float s1 = a1 - dir * (0.95 + 0.3 * hash21(cc + 41.0));
            float s2 = a2 + dir * (0.7 + 0.3 * hash21(cc + 43.0));
            float s3 = a0 - dir * 0.7;
            vec2 m0 = mix(p0, p1, 0.55);
            dr = min(dr, taperSeg(w, p1, p1 + (0.5 + 0.2 * hash21(cc + 47.0)) * vec2(sin(s1), cos(s1)), 0.09, 0.012));
            dr = min(dr, taperSeg(w, p2, p2 + (0.45 + 0.2 * hash21(cc + 53.0)) * vec2(sin(s2), cos(s2)), 0.07, 0.015));
            dr = min(dr, taperSeg(w, m0, m0 + 0.34 * vec2(sin(s3), cos(s3)), 0.07, 0.012));
            dist = min(dist, dr);
        }
    }
    // (Faded out only past where the field is flat: there \`d\` no longer grows.)
    float keep = clamp(gTrue * 4.0, 0.0, 1.0);
    float cover = (1.0 - smoothstep(-aa, aa, dist)) * keep;
    float inner = (1.0 - smoothstep(-lw - aa, -lw + aa, dist)) * keep;
    col = mix(col, INK, cover * 0.92);
    return mix(col, vec3(0.56, 0.41, 0.25), inner);
}

// Slow ripples on water (0-1): light streaks wavering where two drifting noises cross, and now
// and then a drip's rings spreading. \`px\`: squares a pixel.
float ripples(vec2 p, float px) {
    float n1 = vnoise(p * 1.9 + vec2(uTime * 0.11, uTime * 0.05));
    float n2 = vnoise(p * 2.3 + vec2(-uTime * 0.07, uTime * 0.09) + 17.0);
    float streak = (1.0 - smoothstep(0.0, 0.035, abs(n1 - n2))) * smoothstep(0.35, 0.7, vnoise(p * 0.6 + uTime * 0.02));
    // A drip in some cells of two squares, its two rings spreading and fading.
    vec2 sc = p * 0.5;
    vec2 id = floor(sc);
    float ph = fract(uTime * 0.16 + hash21(id + 1.9));
    float dist = length(fract(sc) - (0.35 + 0.3 * vec2(hash21(id + 3.7), hash21(id + 8.1))));
    float lw = max(0.018, px * 0.6);
    float rings = (1.0 - smoothstep(0.0, lw, abs(dist - ph * 0.3))) + 0.6 * (1.0 - smoothstep(0.0, lw, abs(dist - ph * 0.18)));
    rings *= (1.0 - ph) * step(0.55, hash21(id + 5.3));
    return clamp(streak * 0.55 + rings * 0.8, 0.0, 1.0);
}

// One stone of scree to each cell of a grid \`s\` a square (where a cell's hash is under
// \`cover\`): two tones, its NW half lit, its shadow to the SE, inked.
vec3 screeStone(vec3 col, vec2 p, float s, float seed, float cover, float px) {
    vec2 sc = p * s;
    vec2 id = floor(sc);
    float h = hash21(id + seed);
    if (h >= cover) return col;
    vec2 dv = fract(sc) - (0.4 + 0.2 * vec2(hash21(id + seed + 1.3), hash21(id + seed + 2.7)));
    float a = atan(dv.y, dv.x);
    float r = (0.21 + 0.1 * hash21(id + seed + 4.1)) * (1.0 + 0.16 * sin(a * 3.0 + h * 40.0) + 0.07 * sin(a * 5.0 + h * 17.0));
    float len = length(dv);
    float pw = min(px * s, 0.06);
    col = mix(col, col * 0.5, (1.0 - smoothstep(r - pw, r + 0.07, length(dv + uLight * 0.1))) * 0.4);
    vec3 stone = vec3(0.70, 0.67, 0.62) * (0.82 + 0.3 * hash21(id + seed + 5.5));
    stone *= mix(0.66, 1.12, smoothstep(-0.2, 0.2, dot(dv, uLight) / max(r, 1e-3) + 0.3));
    col = mix(col, stone, 1.0 - smoothstep(r - pw, r + pw, len));
    return mix(col, INK, (1.0 - smoothstep(0.0, max(pw * 1.2, 0.05), abs(len - r))) * 0.55);
}

// Terrain on the floor (uTerrain): shallow water over the floor, deep water, a chasm, a rubble
// slope, each from its mask's iso-0.5 bank (roughened by noise, more in rock than in dressed stone).
vec3 terrain(vec3 col, vec2 q, vec2 p, float px) {
    vec4 t = texture(uTerrain, q / uN);
    gTerr = max(max(t.r, t.g), max(t.b, t.a));
    if (gTerr < 0.02) return col;
    float e = 0.25;
    vec4 gx = (texture(uTerrain, (q + vec2(e, 0.0)) / uN) - texture(uTerrain, (q - vec2(e, 0.0)) / uN)) / (2.0 * e);
    vec4 gy = (texture(uTerrain, (q + vec2(0.0, e)) / uN) - texture(uTerrain, (q - vec2(0.0, e)) / uN)) / (2.0 * e);
    vec4 gl = sqrt(gx * gx + gy * gy);
    float wob = (fbm(p * 1.1 + 11.0) - 0.5) * mix(0.12, 0.34, uRough);
    vec4 d = (t - 0.5) / max(gl, vec4(0.35)) + wob * vec4(1.0, 1.0, 0.8, 1.6);
    float aa = max(px * 0.5, 1e-3);
    if (t.r > 0.02) {
        // Shallow water: the floor seen through it, tinted and dimmer toward the middle; wet
        // stone round it, a light line at the edge, inked.
        float ds = d.x;
        float inside = smoothstep(-aa, aa, ds);
        col *= mix(1.0, 0.78, (1.0 - smoothstep(0.0, 0.2, -ds)) * (1.0 - inside));
        vec3 w = col * vec3(0.55, 0.74, 0.76) + vec3(0.04, 0.10, 0.11);
        w *= mix(1.0, 0.84, smoothstep(0.05, 0.7, ds));
        w += ripples(p, px) * vec3(0.30, 0.36, 0.36);
        col = mix(col, w, inside);
        col = mix(col, vec3(0.80, 0.90, 0.88), (1.0 - smoothstep(0.0, max(aa * 2.0, 0.025), abs(ds - 0.05))) * 0.4 * inside);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 2.0, abs(ds))) * 0.5);
    }
    if (t.g > 0.02) {
        // Deep water: dark, lighter where it shelves up to its edge, shadowed under the NW
        // bank; ripples; a wet glint along its rim, inked.
        float dd = d.y;
        float inside = smoothstep(-aa, aa, dd);
        vec3 w = vec3(0.06, 0.17, 0.25) * (0.85 + 0.3 * vnoise(p * 0.7 + vec2(uTime * 0.03, 0.0)));
        w = mix(vec3(0.17, 0.36, 0.41), w, smoothstep(0.0, 0.45, dd));
        w *= mix(0.62, 1.0, smoothstep(0.3, 0.6, texture(uTerrain, (q + uLight * 0.4) / uN).g));
        w += ripples(p, px) * vec3(0.30, 0.45, 0.50);
        col = mix(col, w, inside);
        col = mix(col, vec3(0.62, 0.80, 0.84), (1.0 - smoothstep(0.0, max(aa * 2.0, 0.03), abs(dd - 0.08))) * 0.55);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 2.4, abs(dd))) * 0.8);
    }
    if (t.b > 0.02) {
        // A chasm: a lip of rock round it lit as it faces the NW light, the rock face below it
        // (strata along the edge) falling into black; inked bold at the drop.
        float dc = d.z;
        float inside = smoothstep(-aa, aa, dc);
        vec2 nc = vec2(gx.b, gy.b);
        nc = length(nc) > 1e-4 ? normalize(nc) : vec2(0.0);
        float face = clamp(0.55 + 0.7 * dot(nc, uLight), 0.2, 1.2);
        float lipW = 0.11 + 0.08 * vnoise(p * 2.7 + 5.0);
        float lip = (1.0 - smoothstep(lipW - aa, lipW + aa, -dc)) * (1.0 - inside);
        vec3 lipCol = vec3(0.60, 0.57, 0.52) * (0.85 + 0.3 * vnoise(p * 4.0)) * mix(0.8, 1.15, smoothstep(-0.5, 0.5, dot(nc, uLight)));
        col = mix(col, lipCol, lip * 0.9);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 1.6, abs(dc + lipW))) * 0.55);
        vec3 rf = rockMass(p * 1.7, 0.0) * 2.0 * face;
        rf = mix(rf, rf * 0.5, seam(fract(dc * 4.0 + 0.6 * vnoise(p * 1.5)), 0.06));
        col = mix(col, mix(rf, vec3(0.012, 0.01, 0.008), smoothstep(0.08, 0.75, dc)), inside);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 2.4, abs(dc))) * 0.95);
    }
    if (t.a > 0.02) {
        // A rubble slope: dust over the floor, then scree, small stones spilling out past the
        // big ones at its edge.
        float dr = d.w;
        float cover = smoothstep(-0.25, 0.3, dr);
        // (Over a ramp, thinner: its slope drawn under shows through.)
        float thin = 1.0 - 0.6 * gRamp;
        col = mix(col, vec3(0.56, 0.53, 0.48) * (0.85 + 0.25 * vnoise(p * 5.0)), cover * 0.3 * (1.0 - 0.75 * gRamp));
        col = screeStone(col, p + 0.37, 2.6, 9.0, smoothstep(-0.45, 0.1, dr) * 0.3 * thin, px);
        col = screeStone(col, p, 1.5, 0.0, cover * 0.4 * thin, px);
    }
    return col;
}

// The sprites' outline weights (squares): a shape's outline, a part's.
const float OUT_W = 0.045;
const float LINE_W = 0.025;

// \`c\` stepped in value by \`dv\`, in the square root of its luminance, so that one step reads alike
// on pale flags and dark basalt: ledges take their tops (lighter), faces (mid) and shadows (darker)
// from the theme's own floor tone.
vec3 valueStep(vec3 c, float dv) {
    float y = max(dot(c, vec3(0.3, 0.59, 0.11)), 1e-3);
    float v0 = sqrt(y);
    float v = clamp(v0 + dv, min(v0, 0.03), max(v0, 0.97));
    return min(c * (v * v / y), vec3(1.0));
}

// How dark a floor tone is (1 dark basalt, 0 brown cave rock and paler): on dark floors a ledge's
// top steps up further and its face and shadow further down, so it reads as boldly as in the caves.
float darkness(vec3 c) {
    return 1.0 - smoothstep(0.5, 0.64, sqrt(max(dot(c, vec3(0.3, 0.59, 0.11)), 1e-3)));
}

// A cliff face \`down\` squares below its lip (\`n\`: toward the high side), \`faceW\` wide, stepped down
// from \`base\` (the floor below): a mid tone by how it meets the light, in a few big slanting facets,
// darker toward its foot; broken rock strokes down the fall line, tapered at both ends, of varied
// length and spacing, many starting below the lip (so never even rungs). Along the edge by the
// foot's place (a built edge's x + y; natural rock's the diagonal running most along it).
vec3 cliffFace(vec3 base, vec2 q, vec2 n, float down, float faceW, float px, float aa) {
    float toward = dot(n, normalize(uLight));
    // (The high side toward the light: the face looks away from it, darkest.)
    // (On a dark floor a step further down, but never so near the ink that its strokes are lost.)
    float lit = (toward > 0.35 ? -0.2 : toward > -0.35 ? -0.15 : -0.1) - 0.06 * darkness(base);
    float floorV = 0.36 - sqrt(max(dot(base, vec3(0.3, 0.59, 0.11)), 1e-3));
    vec2 foot = q + n * down;
    vec2 t = vec2(-n.y, n.x);
    float a1 = abs(t.x + t.y);
    float a2 = abs(t.x - t.y);
    bool first = uSquare > 0.5 || a1 >= a2;
    float along = first ? foot.x + foot.y : foot.x - foot.y;
    float rate = uSquare > 0.5 ? 1.0 : max(max(a1, a2), 0.5);
    float k = clamp(down / max(faceW, 1e-3), 0.0, 1.0);
    // Facets: planes of uneven width, slanting down the face.
    float fu = along / rate / 0.55 + 0.6 * down;
    float fid = floor(fu + 0.45 * hash21(vec2(floor(fu * 0.5), 5.1)));
    vec3 fc = valueStep(base, max(lit + 0.05 * (hash21(vec2(fid, 2.9)) - 0.5) - 0.08 * k * k, min(floorV, -0.1)));
    // Strokes: one in most cells along the edge, at a jittered place, slanting a little.
    const float GAP = 0.16;
    float u = along / GAP;
    float id = floor(u);
    float h1 = hash21(vec2(id, 7.7));
    float h2 = hash21(vec2(id, 1.3));
    float h3 = hash21(vec2(id, 3.1));
    float h4 = hash21(vec2(id, 9.4));
    float start = faceW * (h4 < 0.45 ? 0.0 : 0.45 * (h4 - 0.45));
    float len = faceW * (0.3 + 0.65 * h1);
    float sAt = (down - start) / max(len, 1e-3);
    float off = abs((fract(u) - 0.5 - 0.7 * (h2 - 0.5)) * GAP / rate - (h3 - 0.5) * 0.4 * (down - start));
    float hw = LINE_W * 0.6 * clamp(1.0 - sAt, 0.0, 1.0) * clamp(sAt * 5.0, 0.0, 1.0);
    float stroke = (1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, off)) * smoothstep(0.15 * px, 0.6 * px, hw) * step(0.3, fract(h3 * 7.13));
    // (Gone where strokes would crowd within a few pixels, or the face is only a few pixels wide; and
    // on a step's riser, under 0.15 square: plain, only darker at its foot.)
    stroke *= smoothstep(2.5, 4.0, GAP / max(px, 1e-5)) * smoothstep(5.0, 9.0, faceW / max(px, 1e-5)) * smoothstep(0.13, 0.17, faceW);
    return mix(fc, INK, stroke * 0.7);
}

// A built site's ledge face (a gallery's, a stage's, a sunken arena's wall): a dressed stone wall
// seen from above, in one flat tone (\`gFace\`) about the floor's value, lighter where it faces the
// light and a little darker where it looks away (never near the ink), so the lighter top, the lip,
// the face and the soft shadow past it read as a walkway's edge dropping to the floor below, not a
// trough. Its blocks in courses (two on a tall face) in running bond, the joints a darker stone
// tone, not ink. (Risers under 0.15 square, a dais's or a step's: plain, the lip their only line.)
vec3 builtFace(vec2 q, vec2 n, float down, float faceW, float px, float aa) {
    float toward = dot(n, normalize(uLight));
    float k = clamp(down / max(faceW, 1e-3), 0.0, 1.0);
    vec3 fc = valueStep(gFace, mix(0.06, -0.03, smoothstep(-0.6, 0.6, toward)) + 0.02 * (1.0 - k));
    float big = smoothstep(0.13, 0.17, faceW) * smoothstep(6.0, 9.0, faceW / max(px, 1e-5));
    float courses = faceW > 0.3 ? 2.0 : 1.0;
    float ck = k * courses;
    vec2 edge = q + n * down;
    float u = (edge.x + edge.y) / 0.5 + 0.5 * floor(ck);
    float hw = max(0.009, 0.5 * px);
    float dj = abs(fract(u + 0.5) - 0.5) * 0.5;
    float joint = (1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, dj)) * smoothstep(5.0, 8.0, 0.5 / max(px, 1e-5));
    float dc = abs(fract(ck + 0.5) - 0.5) * faceW / courses;
    float course = courses > 1.5 ? 1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, dc) : 0.0;
    return mix(fc, valueStep(fc, -0.16), max(joint, course) * 0.8 * big);
}

// How much lighter a raised floor's top is (a value step, \`valueStep\`), by its height (ft): 2.5 ft
// a touch, 5 ft clearly, 10 ft more, less a foot above that.
float topStep(float ft) {
    return ft > 0.05 ? 0.011 * min(ft, 10.0) + 0.005 * max(ft - 10.0, 0.0) : 0.0;
}

// The face below a lip (squares), for a drop (ft): 2.5 ft 0.1, 5 ft 0.25, 10 ft 0.5, 20 ft 0.8.
float faceWidth(float drop) {
    float w = 0.04 * min(drop, 2.5) + 0.06 * clamp(drop - 2.5, 0.0, 2.5) + 0.05 * clamp(drop - 5.0, 0.0, 5.0) + 0.03 * max(drop - 10.0, 0.0);
    return min(w, 0.8);
}

// Ledges and steps (uLift, \`l\` its texel here), along every edge between floor heights: the high
// side's lip inked at the sprites' outline weight; the cliff face below it (\`cliffFace\`), wider the
// higher the drop; the shadow it casts on the floor below, flat with a soft end, longer and darker
// for bigger drops, strongest away from the light. Toward a ramp (\`l.b\` falling to 0 at it) the
// cliff ends: the lip, at full weight, stops just short of it, the face and shadow narrow to nothing. Natural
// rock's edges wobble as its walls do; built sites' run straight along the squares. First the top
// lightened by its height (\`hf\`, ft; near a natural edge, where the field blurs it, the height a
// little way off on this pixel's own side), so the faces and shadows step down from the floor's tone.
vec3 ledgeEdge(vec3 col, vec4 l, vec2 q, vec2 p, float px, float hf) {
    float r0 = (l.r * 255.0 - 128.0) / 63.5;
    float dk = darkness(col);
    if (r0 > 0.3 || r0 < -1.9 || l.b < 0.01) return valueStep(col, topStep(hf) * (1.0 + 0.8 * dk) * (uSquare > 0.5 ? 1.35 : 1.0));
    // (Toward the high side, from the field's slope.)
    float e = 0.125;
    vec2 g = vec2(texture(uLift, (q + vec2(e, 0.0)) / uN).r, texture(uLift, (q + vec2(0.0, e)) / uN).r) - l.r;
    g *= 255.0 / 63.5 / e;
    float gl = length(g);
    vec2 n = gl > 1e-4 ? g / gl : vec2(0.0);
    float r = r0;
    if (uSquare < 0.5) {
        r0 /= max(gl, 0.35);
        r = r0 + uRough * ((fbm(p * 1.1 + 21.0) - 0.5) * 0.16 + (vnoise(p * 4.3 + 8.0) - 0.5) * 0.05);
    }
    if (uSquare < 0.5) hf = field(q + n * (r0 > 0.0 ? 0.7 : -0.7)).b * 20.0;
    // (Built sites' tops a step lighter again: their floors are darker and busier than cave rock.)
    float topK = (1.0 + 0.8 * dk) * (uSquare > 0.5 ? 1.35 : 1.0);
    col = valueStep(col, topStep(hf) * topK);
    float drop = l.g * 20.0;
    // (Only where the floor really drops: a crossing with no rise is where two floors of one height meet
    // far from any edge, e.g. inside a terrace between a lower and a higher floor; no lip there.)
    float s = l.b * smoothstep(0.1, 0.45, drop);
    float aa = max(px * 0.7, 1e-4);
    float down = -r;
    float faceW = faceWidth(drop) * s;
    // The shadow on the floor below (fac > 0: the high side lies toward the light).
    float fac = smoothstep(-0.3, 0.7, dot(n, normalize(uLight)));
    float shLen = min(0.95, 0.15 + 0.05 * drop) * mix(0.6, 1.0, fac) * s;
    float shV = min(0.34, 0.12 + 0.015 * drop) * mix(0.55, 1.0, fac) * (1.0 + 0.4 * dk);
    float sh = smoothstep(-aa, aa, down) * (1.0 - smoothstep(faceW + shLen * 0.55, faceW + shLen, down));
    // (Built: a short soft shadow on the floor below only, strongest at the face's foot and fading
    // out, slight where the face is lit; darker than the face but never near the ink.)
    if (uSquare > 0.5) {
        shLen = min(0.42, 0.1 + 0.032 * drop) * mix(0.5, 1.0, fac) * s;
        shV = min(0.13, 0.06 + 0.007 * drop) * mix(0.25, 1.0, fac) * (1.0 + 0.2 * dk);
        float st = clamp((down - faceW) / max(shLen, 1e-3), 0.0, 1.0);
        sh = smoothstep(faceW - aa, faceW + aa, down) * (1.0 - st) * (1.0 - st);
    }
    vec3 floorCol = col;
    col = mix(col, valueStep(col, -shV), sh * min(1.0, s * 2.0));
    float inFace = smoothstep(-aa * 0.5, aa * 0.5, down) * (1.0 - smoothstep(faceW - aa * 0.5, faceW + aa * 0.5, down));
    if (uSquare > 0.5) gLedge = max(gLedge, max(inFace, sh) * smoothstep(0.12, 0.3, s));
    if (inFace > 0.0) col = mix(col, uSquare > 0.5 ? builtFace(q, n, -r0, faceW, px, aa) : cliffFace(floorCol, q, n, -r0, faceW, px, aa), inFace);
    // The lip, at full weight up to just short of a ramp, where it ends (bolder on built sites'
    // straight edges, which have no rough wall line to carry them).
    float lw = max(OUT_W * (uSquare > 0.5 ? 1.3 : 1.0), (uSquare > 0.5 ? 1.5 : 1.2) * px);
    float lip = smoothstep(-aa * 0.5, aa * 0.5, r) * (1.0 - smoothstep(lw - aa * 0.5, lw + aa * 0.5, r));
    return mix(col, INK, lip * 0.95 * smoothstep(0.12, 0.3, s));
}

// A ramp (a rubble slope joining two floors; uLift's alpha \`t\` how far up it, 0 at the lower floor's
// edge, 1 at the higher's; its own texel's green the rise between them, its low 3 bits the way up):
// its floor graded from the low floor's tone at its foot to the high one's at its top (\`hf\`, the
// blurred height, half-grades it already) and shaded as it meets the light, with short strokes down
// the fall line from its top edge, broad there and tapering to a point (as the region map's
// hachures), so which way is up reads at a glance. (One fetch more, of its own texel, exact.)
vec3 rampSlope(vec3 col, vec4 l, vec2 q, float px, float hf) {
    float t = clamp((l.a * 255.0 - 1.0) / 254.0, 0.0, 1.0);
    ivec2 sz = textureSize(uLift, 0);
    vec4 e = texelFetch(uLift, clamp(ivec2(floor(q / uN * vec2(sz))), ivec2(0), sz - 1), 0);
    // (A ramp texel's strength is 0; just outside one, only its tone, from the filtered rise.)
    float own = step(e.b, 0.5 / 255.0);
    float code = floor(e.g * 255.0 + 0.5);
    float way = mod(code, 8.0);
    float rise = mix(l.g, (code - way) / 255.0, own) * 20.0;
    vec2 dir = vec2(cos(way * 0.7853982), sin(way * 0.7853982));
    // (Rising toward the light it faces away from it: a little darker.)
    float shade = -0.04 * dot(dir, normalize(uLight)) * own;
    vec3 c = valueStep(col, topStep(hf + (t - 0.5) * 0.5 * rise) * (1.0 + 0.8 * darkness(col)) + shade);
    const float GAP = 0.24;
    float u = dot(q, vec2(-dir.y, dir.x)) / GAP;
    float id = floor(u);
    float h1 = hash21(vec2(id, 4.3));
    float h2 = hash21(vec2(id, 8.1));
    float h3 = hash21(vec2(id, 2.2));
    float len = 0.4 + 0.35 * h1;
    float sAt = (1.0 - t) / len;
    float off = (fract(u) - 0.5 - 0.5 * (h2 - 0.5)) * GAP - (h3 - 0.5) * 0.2 * (1.0 - t);
    float hw = 0.5 * max(0.05, 1.6 * px) * clamp(1.0 - sAt, 0.0, 1.0);
    float aa = max(px * 0.7, 1e-4);
    float stroke = (1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, abs(off))) * step(0.2, fract(h3 * 5.31));
    stroke *= smoothstep(2.0, 0.8, sAt) * smoothstep(3.0, 5.0, GAP / max(px, 1e-5)) * step(0.5, rise) * own;
    return mix(c, INK, stroke * 0.8);
}

// The floor's height drawn (\`ledgeEdge\`), and over a ramp its slope (\`rampSlope\`).
vec3 ledges(vec3 col, vec4 l, vec2 q, vec2 p, float px, float hf) {
    vec3 c = ledgeEdge(col, l, q, p, px, hf);
    gRamp = step(0.5 / 255.0, l.a) * (1.0 - smoothstep(0.03, 0.2, l.b));
    if (gRamp > 0.0) c = mix(c, rampSlope(col, l, q, px, hf), gRamp);
    return c;
}

void main() {
    vec2 q = vUV * (uN + 2.0 * uGrow) - uGrow;
    vec2 p = q + uOrigin;
    // (Squares a pixel.)
    float px = length(fwidth(q));
    gPx = px;
    int style = int(uTheme + 0.5);
    // Squares to the nearest floor: past where the rock has faded out, nothing.
    float far = texture(uDist, (q + DIST_PAD) / (uN + 2.0 * DIST_PAD)).r * DIST_MAX;
    if (far > 4.2) {
        finalColor = vec4(0.0);
        return;
    }
    vec4 here = field(q);
    // Ledges and steps.
    vec4 lift = vec4(0.5, 0.0, 0.0, 0.0);
    if (uHasLift > 0.5) lift = texture(uLift, q / uN);
    float d;
    vec2 n;
    if (uSquare > 0.5) {
        boxField(q, d, n);
    } else {
        // Signed distance to the wall (squares, + on the floor) from the blurred floor field.
        float e = 0.25;
        vec2 grad = vec2(field(q + vec2(e, 0.0)).r - field(q - vec2(e, 0.0)).r, field(q + vec2(0.0, e)).r - field(q - vec2(0.0, e)).r) / (2.0 * e);
        d = (here.r - 0.5) / max(length(grad), 0.35);
        gTrue = smoothstep(0.12, 0.3, length(grad));
        // Rock juts out and bites in (caves); dressed stone stays nearly straight.
        d += uRough * ((fbm(p * 0.8 + 3.0) - 0.5) * 0.75 + (vnoise(p * 3.7) - 0.5) * 0.25 + (vnoise(p * 9.0) - 0.5) * 0.08);
        n = length(grad) > 1e-4 ? normalize(grad) : vec2(0.0);
    }
    // Near shaped rooms and caverns: the floor drawn from them too (the farther in wins).
    int won = 0;
    vec3 look = vec3(0.0);
    float rough = uRough;
    // (Where the floor's edge casts its shadow from: the floor there.)
    vec2 qs = q + uLight * 0.45;
    float lit = field(qs).r;
    ivec2 s0 = clamp(ivec2(floor(q)), ivec2(0), ivec2(uN) - 1);
    // (Only on a shape's squares or those next to one: uCells.g is 0 elsewhere.)
    if ((uShapeCount > 0.5 || uCaverns > 0.5) && texelFetch(uCells, s0, 0).g > 0.5 / 255.0) {
        // The shapes round (four at most), a cavern among them.
        int slots[4] = int[4](0, 0, 0, 0);
        int found = 0;
        bool cave = false;
        for (int j = -1; j <= 1; j++) {
            for (int i = -1; i <= 1; i++) {
                ivec2 c = s0 + ivec2(i, j);
                if (any(lessThan(c, ivec2(0))) || any(greaterThanEqual(c, ivec2(uN)))) continue;
                int g = int(texelFetch(uCells, c, 0).g * 255.0 + 0.5);
                if (g == 255) cave = true;
                if (g == 0 || g >= 254 || found == 4) continue;
                bool seen = false;
                for (int m = 0; m < 4; m++) seen = seen || (m < found && slots[m] == g);
                if (!seen) {
                    slots[found] = g;
                    found++;
                }
            }
        }
        for (int k = 0; k < 4; k++) {
            if (k >= found) break;
            int slot = slots[k];
            vec2 lo;
            vec2 hi;
            vec3 lk;
            shapeSlot(slot, lo, hi, lk);
            int code = int(lk.x);
            float ds = shapeIn(code, q, lo, hi);
            lit = max(lit, smoothstep(-0.05, 0.05, shapeIn(code, qs, lo, hi)));
            if (ds > d) {
                float e = 0.03;
                vec2 gs = vec2(shapeIn(code, q + vec2(e, 0.0), lo, hi) - shapeIn(code, q - vec2(e, 0.0), lo, hi), shapeIn(code, q + vec2(0.0, e), lo, hi) - shapeIn(code, q - vec2(0.0, e), lo, hi));
                d = ds;
                gTrue = 1.0;
                n = length(gs) > 1e-6 ? normalize(gs) : vec2(0.0);
                won = slot;
                look = lk;
            }
        }
        if (cave && all(greaterThanEqual(q, vec2(0.0))) && all(lessThan(q, uN))) {
            float e = 0.25;
            float c0 = texture(uCave, q / uN).r;
            vec2 gc = vec2(texture(uCave, (q + vec2(e, 0.0)) / uN).r - texture(uCave, (q - vec2(e, 0.0)) / uN).r, texture(uCave, (q + vec2(0.0, e)) / uN).r - texture(uCave, (q - vec2(0.0, e)) / uN).r) / (2.0 * e);
            float dc = (c0 - 0.5) / max(length(gc), 0.35);
            dc += (fbm(p * 0.8 + 3.0) - 0.5) * 0.75 + (vnoise(p * 3.7) - 0.5) * 0.25 + (vnoise(p * 9.0) - 0.5) * 0.08;
            lit = max(lit, smoothstep(0.35, 0.6, texture(uCave, qs / uN).r));
            if (dc > d) {
                d = dc;
                gTrue = smoothstep(0.12, 0.3, length(gc));
                n = length(gc) > 1e-4 ? normalize(gc) : vec2(0.0);
                won = 255;
                rough = 1.0;
            }
        }
    }
    float aa = max(fwidth(d), 1e-4);

    vec3 col;
    float alpha;
    float rim = mix(0.16, 0.3, rough);
    // (A dwarven hall's walls are thick: a wider coping, inked on both sides.)
    if (style == 1 && rough < 0.2) rim = 0.3;
    if (d < 0.0) {
        // Rock: the wall's top edge lit from the NW, then the rock hatched beside the walls,
        // fading out beyond (the map above shows round the site). (Near the wall its depth is
        // the wall's own; farther out, from uDist.)
        float depth = mix(-d, far, smoothstep(0.6, 1.4, far));
        float cut;
        float ra;
        vec3 r = hatchedRock(p, depth, px, cut, ra);
        float face = clamp(0.55 + 0.6 * dot(n, uLight), 0.25, 1.15);
        // (Natural rock's lip in the site's own rock: caves as they were.)
        vec3 lip = rockMass(p * 1.3, 0.0) * 1.55 * face * (uRock / vec3(0.29, 0.27, 0.245));
        // (Ice: a flat pale-blue edge, shaded by the light, in place of the dark rock's.)
        if (uLook > 3.5) lip = mix(uRock * 1.2, vec3(0.88, 0.94, 0.98), 0.55) * (0.8 + 0.3 * face);
        float edge = rim;
        if (rough < 0.2) {
            // Dressed stone coping on built walls, by the site's style.
            vec2 b;
            vec2 g;
            float jw = 0.06;
            float jk = 0.5;
            if (style == 2) {
                // Rough ashlar: uneven blocks, each course shifted its own way, wavering joints,
                // a ragged edge toward the rock.
                b = q * vec2(1.3, 1.7);
                b.x += hash21(vec2(floor(b.y), 2.7)) + 0.3 * (vnoise(p * 2.1) - 0.5);
                b.y += 0.15 * (vnoise(p * 2.3 + 7.0) - 0.5);
                g = fract(b);
                jw = 0.09;
                edge = rim + 0.1 * (vnoise(p * 2.7 + 1.0) - 0.5);
            } else if (style == 5) {
                // Brick: stretchers in running bond.
                b = q * vec2(2.2, 4.4);
                b.x += 0.5 * mod(floor(b.y), 2.0);
                g = fract(b);
                jw = 0.1;
                jk = 0.4;
            } else {
                float k = style == 1 ? 0.8 : style == 6 ? 1.0 : 1.6;
                b = q * k;
                g = fract(b + 0.5 * mod(floor(b.yx), 2.0));
                jk = style == 6 ? 0.22 : style == 4 ? 0.3 : style == 7 ? 0.0 : style == 1 ? 0.85 : 0.5;
                jw = style == 1 ? 0.14 : 0.06;
            }
            float joint = 1.0 - smoothstep(0.0, jw, min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y)));
            vec3 cop = uCoping * (0.85 + 0.25 * hash21(floor(b)));
            if (style == 2) {
                // (A few blocks chipped dark.)
                cop *= hash21(floor(b) + 5.5) < 0.2 ? 0.78 : 1.0;
            } else if (style == 4) {
                // (Dust over it.)
                cop = mix(cop, vec3(0.76, 0.73, 0.66), smoothstep(0.45, 0.8, vnoise(p * 1.7)) * 0.4);
            } else if (style == 5) {
                // (Damp, green toward the floor.)
                cop = mix(cop, cop * vec3(0.62, 0.84, 0.5), max(smoothstep(0.4, 0.75, vnoise(p * 0.9)) * 0.6, (1.0 - smoothstep(0.02, 0.2, -d)) * 0.9));
            } else if (style == 7) {
                // (Packed earth: no joints, lumpy.)
                cop = uCoping * (0.82 + 0.3 * vnoise(p * 1.6));
            }
            // (Lit, toned down: a pale coping would be the brightest thing in the room.)
            lip = mix(cop * face * (1.0 - 0.15 * smoothstep(0.7, 1.15, face)), INK, joint * jk);
        }
        col = mix(lip, r, smoothstep(-edge, -edge - 0.12, d));
        if (style == 1 && rough < 0.2) col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 1.6, abs(d + rim))) * 0.75);
        // (Ice rock reaches a little farther before it fades, filling the pockets between rooms.)
        float fade = uLook > 3.5 ? 3.1 : 2.2;
        alpha = ra * (1.0 - smoothstep(fade, fade + 1.0, depth + cut));
        // (Dressed coping is solid stone, not see-through as the hatched rock beyond it is: else the
        // map above showed through a lit wall's top as a pale haze, and round an outer corner as a
        // smeared wedge.)
        if (rough < 0.2) alpha = mix(1.0, alpha, smoothstep(-edge, -edge - 0.12, d));
    } else {
        ivec2 s = clamp(ivec2(floor(q)), ivec2(0), ivec2(uN) - 1);
        vec4 cell = texelFetch(uCells, s, 0) * 255.0;
        if (won > 0 && won < 255) {
            // A shaped room's floor, wherever its shape reaches.
            // (A sunken arena's sand keeps its squares.)
            cell = vec4(cell.r > 11.5 && cell.a > 0.5 ? 12.0 : look.y, 0.0, look.z, 255.0);
        } else if (won == 255) {
            cell = vec4(0.0, 255.0, cell.b, 255.0);
        } else if (cell.a < 0.5) {
            // A smoothed edge reaching into a rock square: the floor next to it.
            cell = texelFetch(uCells, clamp(s + ivec2(round(n)), ivec2(0), ivec2(uN) - 1), 0) * 255.0;
            // (That floor may lie past the section's edge: sewers are all brick.)
            // (The city's stonework is on the rock too.)
            if (cell.a < 0.5 && uPad > 0.5) cell = vec4(3.0, 0.0, cell.b, 255.0);
        }
        int mat = int(cell.r + 0.5);
        col = floorColor(mat, q, p, cell.b);
        if (style == 4) {
            // Dust on a tomb's flags, drifted against the walls.
            float dust = smoothstep(0.5, 0.9, vnoise(p * 0.6 + 40.0) + 0.35 * (1.0 - smoothstep(0.0, 1.2, d)));
            col = mix(col, vec3(0.75, 0.71, 0.63), dust * 0.35 * step(3.5, float(mat)));
        } else if (style == 2) {
            // A fallen fortress's flags, cracked.
            col = mix(col, col * 0.7, seam(vnoise(p * 0.7 + 17.0), 0.012) * 0.8 * step(3.5, float(mat)));
        } else if (style == 3) {
            // The cult's floor, a dark red stain spreading here and there.
            col = mix(col, col * vec3(0.86, 0.7, 0.68), smoothstep(0.55, 0.8, vnoise(p * 0.4 + 23.0)) * 0.5 * step(3.5, float(mat)));
        }
        if (uLook > 0.5 && uLook < 3.5) col = cave_look(col * uTint, p, d, n);
        if (mat <= 2) {
            // Natural floors: uneven rock lit from the NW, loose stones, rubble at the wall foot.
            float b0 = vnoise(p * 0.7);
            vec2 bg = vec2(vnoise(p * 0.7 + vec2(0.12, 0.0)) - b0, vnoise(p * 0.7 + vec2(0.0, 0.12)) - b0);
            // (Softly: clamped hard, its lit and shaded sides met in straight-edged bands along the noise's lattice.)
            float rl = dot(bg, uLight) / 0.06;
            col *= 1.0 + 0.15 * rl / sqrt(1.0 + rl * rl);
            vec2 sc = q * 1.7;
            vec2 cid = floor(sc);
            vec2 off = vec2(hash21(cid), hash21(cid + 7.3)) * 0.6 + 0.2;
            float pr = 0.09 + 0.08 * hash21(cid + 3.1);
            vec2 dv = fract(sc) - off;
            if (hash21(cid + 1.7) > 0.72) {
                float stone = 1.0 - smoothstep(pr - 0.03, pr, length(dv));
                float shade = 1.0 - smoothstep(pr * 0.4, pr + 0.06, length(dv + uLight * pr * 0.6));
                col = mix(col, col * 0.55, (1.0 - smoothstep(pr, pr + 0.07, length(dv - uLight * 0.05))) * 0.6);
                col = mix(col, col * (1.05 + 0.25 * shade), stone);
            }
            float foot = (1.0 - smoothstep(0.05, 0.6, d)) * rough;
            float rub = smoothstep(0.45, 0.75, vnoise(p * 4.5 + 13.0));
            col = mix(col, mix(col * 0.6, col * 1.2, vnoise(p * 9.0)), foot * rub * 0.8);
        }
        // Raised floors: tops lighter by their height; their edges' lips, faces and shadows.
        float hf = here.b * 20.0;
        if (uSquare > 0.5) {
            vec3 boss = uStone * vec3(0.76, 0.71, 0.74);
            gFace = mat == 7 ? vec3(0.70, 0.68, 0.65) : mat == 10 ? vec3(0.30, 0.28, 0.30) : mat == 5 || mat == 12 ? boss : mat == 9 ? col : uStone * 0.92;
        }
        if (uHasLift > 0.5) col = ledges(col, lift, q, p, px, hf);
        else col = valueStep(col, topStep(hf));
        // Water, chasms, rubble slopes.
        if (uHasTerrain > 0.5) col = terrain(col, q, p, px);
        // Against the walls: ambient occlusion and the shadow cast from the NW.
        // (A built gallery's top, up by the wall's head, keeps only a thin band of it and none of the
        // wall's cast shadow: else a strip a square wide is all shade and its lighter top is lost.)
        float up = uSquare > 0.5 ? max(smoothstep(2.0, 8.0, hf), gLedge) : 0.0;
        col *= mix(0.6 + 0.4 * smoothstep(0.0, 0.75, d), 0.9 + 0.1 * smoothstep(0.0, 0.2, d), up);
        col *= mix(mix(0.66, 1.0, smoothstep(0.35, 0.6, lit)), 1.0, up);
        // (Roots over the wall's shade, so they keep their tone; not over water, a chasm or a slope.)
        if (uLook > 0.5 && uLook < 1.5) col = mix(col, caveRoots(col, p, d, n, px), (1.0 - smoothstep(0.02, 0.3, gTerr)) * (1.0 - gRamp));
        // Liquid down the channel.
        float l = here.g;
        if (uLiquid > 0.5) {
            float lw = max(fwidth(l), 1e-3);
            float inside = smoothstep(0.5 - lw, 0.5 + lw, l);
            vec3 liq;
            if (uLiquid < 1.5) {
                // Sewage, painted: three flat tones (darker down the middle), a few soft strokes
                // drifting downstream, floating scum and the odd bit of debris, foam where runs
                // meet or end; slime and damp brick along its edge and the wall foot, puddles.
                float wob = vnoise(p * 0.8 + 31.0);
                float lt = l + 0.03 * (wob - 0.5);
                float sw = max(lw, 0.006);
                liq = mix(vec3(0.32, 0.37, 0.20), vec3(0.26, 0.31, 0.16), smoothstep(0.565 - sw, 0.565 + sw, lt));
                liq = mix(liq, vec3(0.21, 0.26, 0.13), smoothstep(0.625 - sw, 0.625 + sw, lt));
                // A thin wet glint just inside each edge (matte, painted).
                liq = mix(liq, vec3(0.47, 0.52, 0.34), (1.0 - smoothstep(0.0, max(lw * 1.2, 0.005), abs(lt - 0.512))) * 0.7);
                // Along and across the run (\`flowAngle\`; the strokes fade where runs meet).
                float ph = (here.a - 0.5) * 3.14159265;
                vec2 fd = cos(ph) * vec2(0.958, 0.287) + sin(ph) * vec2(-0.287, 0.958);
                vec2 fp = vec2(dot(p, fd), dot(p, vec2(-fd.y, fd.x)));
                float calm = (1.0 - smoothstep(0.3, 0.7, fwidth(here.a) / max(px, 1e-4))) * smoothstep(0.54, 0.6, l);
                vec2 sc = vec2(fp.x * 0.45 - uTime * 0.13, fp.y * 4.0);
                sc.x += 5.0 * hash21(vec2(floor(sc.y), 3.1));
                vec2 sid = floor(sc);
                vec2 sf = fract(sc);
                float stroke = smoothstep(0.0, 0.3, sf.x) * (1.0 - smoothstep(0.5, 0.95, sf.x));
                stroke *= 1.0 - smoothstep(0.1, 0.24, abs(sf.y - 0.5 + 0.12 * sin(sf.x * 6.28 + sid.x)));
                stroke *= step(0.55, hash21(sid + 0.7)) * calm * (1.0 - smoothstep(0.06, 0.12, px));
                // (Foam in steps of 8, the city's stonework in the low bits: \`sewerFoam\`.)
                float stw = mod(cell.b + 0.5, 8.0) - 0.5;
                // (Blended between squares, so froth has no square edge: four fetches, in the channel only.)
                float fo = 0.0;
                if (uPad > 0.5 && mat == 3 && l > 0.48) {
                    vec2 fq = q - 0.5;
                    ivec2 f0 = ivec2(floor(fq));
                    vec2 fw4 = fract(fq);
                    ivec2 fm = ivec2(uN) - 1;
                    vec4 fs = vec4(texelFetch(uCells, clamp(f0, ivec2(0), fm), 0).b, texelFetch(uCells, clamp(f0 + ivec2(1, 0), ivec2(0), fm), 0).b,
                                   texelFetch(uCells, clamp(f0 + ivec2(0, 1), ivec2(0), fm), 0).b, texelFetch(uCells, clamp(f0 + ivec2(1, 1), ivec2(0), fm), 0).b) * 255.0;
                    fs = max(fs - mod(fs + 0.5, 8.0) + 0.5, 0.0) / 248.0;
                    fo = mix(mix(fs.x, fs.y, fw4.x), mix(fs.z, fs.w, fw4.x), fw4.y);
                }
                // (No strokes in the froth, where the runs' ways meet and they'd break up.)
                stroke *= 1.0 - smoothstep(0.1, 0.4, fo);
                liq = mix(liq, liq * 1.25 + vec3(0.03, 0.04, 0.02), stroke * 0.7);
                // Scum and debris, drifting more slowly (cells 2 x 0.6 squares).
                vec2 bc = vec2(fp.x * 0.5 - uTime * 0.06, fp.y * 1.6);
                vec2 bid = floor(bc);
                float bh = hash21(bid + 2.3);
                vec2 bv = (fract(bc) - vec2(0.3 + 0.4 * hash21(bid + 5.1), 0.5)) * vec2(2.0, 0.625);
                float deb = step(0.94, bh);
                // (Two soft lobes, smooth-joined; debris a single sliver.)
                float ba = 6.2832 * hash21(bid + 3.7);
                vec2 bo = vec2(cos(ba), sin(ba));
                float br = mix(0.12 + 0.04 * hash21(bid + 8.7), 0.09, deb);
                vec2 be = bv * vec2(1.0, 1.0 + deb);
                float bd1 = length(be - bo * 0.05 * (1.0 - deb)) - br;
                float bd2 = length(be + bo * 0.09) - br * 0.68 + deb;
                float bk = clamp(0.5 + 0.5 * (bd2 - bd1) / 0.06, 0.0, 1.0);
                float bd = mix(bd2, bd1, bk) - 0.06 * bk * (1.0 - bk);
                float bew = max(px, 0.015);
                float blob = (1.0 - smoothstep(-bew, bew, bd)) * step(0.72, bh) * calm;
                vec2 bg = bv.x * fd + bv.y * vec2(-fd.y, fd.x);
                vec3 scum = mix(vec3(0.44, 0.45, 0.31), vec3(0.55, 0.55, 0.40), smoothstep(-0.3, 0.6, -dot(bg, uLight) / br));
                scum = mix(scum, vec3(0.30, 0.22, 0.14), deb);
                liq = mix(liq, scum, blob * 0.8);
                // Foam where runs meet, end or fall out (sewer sections' brick only): soft cream
                // patches merging along the falling water, fading out softly.
                // (Where runs meet the way turns within a texel, and \`fp\`, far from the origin, swings
                // through many noise cells a pixel: there the noise fades to its mean, or it speckles.)
                vec2 fc = vec2(fp.x * 2.2 - uTime * 0.3, fp.y * 2.8);
                vec2 fcw = fwidth(fc);
                float fsharp = 1.0 - smoothstep(0.25, 0.6, max(fcw.x, fcw.y));
                if (fo > 0.0) {
                    // (Value noise from hashes, drifting downstream, roughened by \`wob\`; no fetches.)
                    vec2 fi = floor(fc);
                    vec2 ff = fract(fc);
                    ff = ff * ff * (3.0 - 2.0 * ff);
                    float fn = mix(mix(hash21(fi), hash21(fi + vec2(1.0, 0.0)), ff.x), mix(hash21(fi + vec2(0.0, 1.0)), hash21(fi + 1.0), ff.x), ff.y);
                    fn = 0.75 * mix(0.5, fn, fsharp) + 0.25 * wob;
                    float fv = 1.2 * (fn - 0.5) + 1.1 * fo - 0.55;
                    float fw = 0.06 + 0.5 * px;
                    float fpat = smoothstep(-fw, fw, fv) * smoothstep(0.52, 0.56, l) * smoothstep(0.0, 0.2, fo);
                    vec3 cream = liq * 1.05 + vec3(0.16, 0.14, 0.10);
                    // (Lit from the upper left: lighter on the side toward the light.)
                    cream += vec3(0.03) * clamp(-dot(fd, uLight) * fv * 4.0, -1.0, 1.0);
                    liq = mix(liq, cream, fpat * 0.85);
                }
                // Kerb stones along the edge (sewer sections): a lighter dressed course in stones
                // a square or so long, the lip toward the water shaded, chipped here and there.
                if (uPad > 0.5 && mat == 3) {
                    float kw = max(lw, 0.004);
                    float kerb = smoothstep(0.26 - kw, 0.26 + kw, l) * (1.0 - inside);
                    float ku = fp.x * 0.85;
                    float kid = floor(ku);
                    float kf = fract(ku);
                    float kh = hash21(vec2(kid, 7.7));
                    float kwarm = step(3.5, stw);
                    vec3 kc = mix(vec3(0.63, 0.61, 0.57), vec3(0.72, 0.65, 0.53), kwarm) * (0.92 + 0.08 * floor(kh * 2.999));
                    // The lip: the stone's face dropping to the water.
                    kc *= mix(1.0, 0.8, smoothstep(0.47, 0.49, l));
                    // Chips: a bite or two out of the lip.
                    float cx = kf - 0.25 - 0.5 * hash21(vec2(kid, 2.9));
                    float chip = step(0.55, kh) * (1.0 - smoothstep(0.0, 0.03, length(vec2(cx * 1.4, (0.5 - l) * 2.5)) - 0.14 - 0.03 * sin(kf * 40.0)));
                    kc = mix(kc, col * 0.78, chip);
                    float kcalm = 1.0 - smoothstep(0.6, 1.2, fwidth(here.a) / max(px, 1e-4));
                    float kj = (1.0 - smoothstep(0.0, 0.04 + 1.2 * fwidth(ku), min(kf, 1.0 - kf))) * kcalm * (1.0 - smoothstep(0.12, 0.25, fwidth(ku)));
                    kc = mix(kc, kc * 0.66, kj * 0.8);
                    // (Its inner edge: a mid-tone joint on the walkway.)
                    kc = mix(kc, col * 0.7, (1.0 - smoothstep(0.0, kw * 2.5, l - 0.26)) * 0.7);
                    col = mix(col, kc, kerb);
                }
                // On the walkway: damp stone toward the water, a soft slime band at its edge, slime
                // at the wall foot, a few puddles.
                col *= mix(1.0, 0.86, smoothstep(0.06, 0.4, l));
                col = mix(col, vec3(0.36, 0.40, 0.22), smoothstep(0.44 + 0.04 * wob, 0.49, l) * 0.4);
                col = mix(col, vec3(0.31, 0.35, 0.21), (1.0 - smoothstep(0.05, 0.16 + 0.14 * wob, d)) * 0.5);
                vec2 pc = p * 0.5;
                vec2 pid = floor(pc);
                vec2 pv = (fract(pc) - 0.3 - 0.4 * vec2(hash21(pid + 7.7), hash21(pid + 2.9))) * 2.0;
                float pr = (0.26 + 0.12 * hash21(pid + 3.3)) * (1.0 + 0.15 * sin(pv.x * 9.0 + pv.y * 7.0));
                float pud = (1.0 - smoothstep(pr - max(px, 0.015), pr + max(px, 0.015), length(pv * vec2(1.0, 1.4)))) * step(0.8, hash21(pid + 0.4));
                pud *= (1.0 - smoothstep(0.12, 0.3, l)) * smoothstep(0.18, 0.35, d);
                vec3 wetCol = col * vec3(0.76, 0.80, 0.80) + vec3(0.03, 0.04, 0.04);
                wetCol = mix(wetCol, vec3(0.72, 0.78, 0.74), (1.0 - smoothstep(0.0, 0.08, length(pv + uLight * pr * 0.55) - pr * 0.3)) * 0.35);
                col = mix(col, wetCol, pud);
            } else {
                float n1 = vnoise(p * 1.2 + vec2(uTime * 0.05, uTime * 0.03));
                liq = mix(vec3(0.92, 0.26, 0.05), vec3(1.0, 0.78, 0.25), n1);
                float crust = smoothstep(0.58, 0.7, vnoise(p * 2.3 + uTime * 0.04));
                liq = mix(liq, vec3(0.16, 0.08, 0.05), crust * 0.85);
                // Glow on the floor beside it.
                col += vec3(0.5, 0.17, 0.03) * smoothstep(0.08, 0.5, l) * 0.7;
            }
            col = mix(col, liq, inside);
            col = mix(col, INK, (1.0 - smoothstep(0.0, lw * 2.0, abs(l - 0.5))) * 0.55);
        }
        // The 5-ft grid.
        if (uGhost < 0.5 && uGrid > 0.0) {
            vec2 gq = min(fract(q), 1.0 - fract(q));
            float gw = length(fwidth(q));
            // (Light over what is nearly black: a chasm, deep water.)
            vec3 gc = mix(vec3(0.45, 0.43, 0.40), INK, smoothstep(0.05, 0.16, dot(col, vec3(0.3, 0.59, 0.11))));
            // Like the surface grid: 0.4-1.2 px soft line.
            float gpx = min(gq.x, gq.y) / max(gw, 1e-5);
            col = mix(col, gc, (1.0 - smoothstep(0.4, 1.2, gpx)) * uGridAlpha);
        }
        alpha = 1.0;
    }
    // The wall's inked outline.
    col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 1.6, abs(d))) * 0.85);
    // A neighbouring section: its floors and their rims only, faint.
    if (uGhost > 0.5) alpha *= d < -rim ? 0.0 : 0.42;
    finalColor = vec4(col * alpha, alpha) * uColor;
}
`,X=new Set([`tunnel`,`cobweb`,`ore_vein`,`niche`,`chains`,`sconce`,`banner`,`pipe`,`ladder`]),Ii=new Set([`dais`,`collapsed_wall`,`throne`,`steps`,`climb_ladder`,`arcane_workbench`]),Li=new Set([`throne`]),Ri=new Set([`sewage`,`lava`,`rail`,`guano`,`moss`,`rubble`,`debris`,`sulfur`,`drain`,`grille`]),zi=new Set([`drain`,`grille`]),Bi=[[0,-1],[1,0],[0,1],[-1,0]],Vi=e=>e.back!==void 0&&e.back>=0&&e.back<4?Bi[e.back]:null,Hi=([e,t])=>!e&&!t?null:Math.abs(t)>=Math.abs(e)?t<0?0:2:e>0?1:3,Ui={chest:`chest`,barrel:`barrel`,crate:`crate`,table:`table`,rug:`rug`,altar:`altar`,statue:`statue`,pillar:`pillar`,cage:`cage`,rack:`rack`,weapon_rack:`weapon_rack`,cot:`cot`,sarcophagus:`sarcophagus`,winch:`winch`,trapdoor:`trapdoor`,bookshelf:`bookcase`,desk:`desk`,long_table:`long_table`,vat:`vat`,anvil:`anvil`,forge:`forge`,hearth:`hearth`,keg_rack:`keg_rack`,chair_of_office:`chair_of_office`};function Wi(e,t){return t?Ui[e]:te.has(e)||!ae.has(e)||Li.has(e)?e:void 0}var Gi=new Set([`boulder`,`guano`,`moss`,`rubble`,`debris`,`sulfur`,`ore_vein`,`nest`,`rat_nest`]),Ki=[10130827,14172463,15239466,15256618,4173394,3114712,10114772,15789280],qi=e=>{if(e>=0&&e<Ki.length)return Ki[e];if(e>31)return Ki[0];let t=e-Ki.length;return Ji((t*47+20)%360/360,.62,[.5,.36,.64][t%3])};function Ji(e,t,n){let r=t*Math.min(n,1-n),i=t=>{let i=(t+e*12)%12;return Math.round(255*(n-r*Math.max(-1,Math.min(i-3,9-i,1))))};return i(0)<<16|i(8)<<8|i(4)}var Yi=new Set([`water_shallow`,`water_deep`,`chasm`,`rubble_slope`]),Xi=new Set([`rope_bridge`,`stone_bridge`,`stepping_stone`]),Z=(e,t,n)=>{let r=Math.sin(e*127.1+t*311.7+n*74.7)*43758.5453;return r-Math.floor(r)};function Zi(e,t){let n=n=>{let r=e>>n&255;return Math.round(t<1?r*t:r+(255-r)*(t-1))&255};return n(16)<<16|n(8)<<8|n(0)}var Q=[3422270,7107452,11186875];function Qi(e,t,n,r,i){let[a,o,s]=[.36,.27,.105],c=n-.06,l=i?.1:0,u=c-.06-l,d=(n,r)=>{e.moveTo(t+s,c+.02).lineTo(t+s,u).arc(t,u,s,0,Math.PI,!0).lineTo(t-s,i?u+.05:c+.02).stroke({width:n,color:r,cap:`round`})};d(.095,N),d(.055,13225684),e.arc(t,u,s,Math.PI*1.1,Math.PI*1.45).stroke({width:.018,color:16054008,cap:`round`}),e.roundRect(t-a/2,c,a,o,.06).fill(Zi(r,.72)),e.roundRect(t-a/2+.02,c+.015,a-.04,o*.62,.05).fill(r),e.moveTo(t-a/2+.06,c+.04).lineTo(t+a/2-.08,c+.04).stroke({width:.022,color:Zi(r,1.45),cap:`round`}),e.roundRect(t-a/2,c,a,o,.06).stroke({width:.035,color:N}),e.circle(t,c+.11,.034).fill(N),e.poly([t-.018,c+.12,t+.018,c+.12,t+.03,c+.205,t-.03,c+.205]).fill(N)}function $i(e,t,n,r){e.circle(t,n,.47).fill({color:r,alpha:.13}).stroke({width:.035,color:r,alpha:.6});let[i,a]=[Math.cos(-.45)*1.3,Math.sin(-.45)*1.3],o=(e,r)=>[t+.02+e*i-r*a,n+e*a+r*i],[s,c]=o(-.2,.2),l=.5,u=(e,t)=>[s+(e*Math.cos(l)-t*Math.sin(l))*1.25,c+(e*Math.sin(l)+t*Math.cos(l))*1.25];e.moveTo(...o(-.2,.06)).lineTo(...u(-.09,0)).stroke({width:.018,color:N});let d=[...u(-.1,0),...u(-.04,-.07),...u(.13,-.07),...u(.13,.07),...u(-.04,.07)];e.poly(d).fill(Zi(r,.75)),e.poly([...u(-.07,-.01),...u(-.03,-.055),...u(.11,-.055),...u(.11,.02),...u(-.03,.02)]).fill(r),e.poly(d).stroke({width:.025,color:N,join:`round`}),e.circle(...u(-.035,0),.016).fill(N);let f=(t,n)=>{e.circle(...o(-.2,0),.1365).stroke({width:t,color:n}),e.moveTo(...o(-.1,0)).lineTo(...o(.3,0)).stroke({width:t,color:n,cap:`round`}),e.moveTo(...o(.2,0)).lineTo(...o(.2,.11)).moveTo(...o(.29,0)).lineTo(...o(.29,.13)).stroke({width:t*.95,color:n,cap:`square`})};f(.12,N),f(.068,Q[1]),e.arc(...o(-.2,0),.1365,-.45+Math.PI*1.05,-.45+Math.PI*1.6).stroke({width:.024,color:Q[2]}),e.moveTo(...o(-.08,-.014)).lineTo(...o(.27,-.014)).stroke({width:.022,color:Q[2],cap:`round`})}function ea(e,t,n,r,i=9){let a=[],o=Z(r,3,7)*Math.PI*2;for(let s=0;s<i;s++){let c=o+s/i*Math.PI*2,l=n*(.84+.3*Z(r,s,11));a.push(e+Math.cos(c)*l,t+Math.sin(c)*l*.9)}return a}var ta=[`no cover`,`half cover`,`three-quarters cover`,`total cover`],na=g.from({vertex:Zt,fragment:Fi,name:`underground`}),ra=.62;function ia(e,t,n){let r=e=>t>=e.x&&t<e.x+e.w&&n>=e.y&&n<e.y+e.h;return e.furniture.find(e=>r(e)&&!be.has(e.kind))??e.furniture.find(r)}function aa(e,t,n=4){let r=Math.sin(e*127.1+t*311.7)*43758.5453;return Math.floor((r-Math.floor(r))*n)}var oa=new Set([`wall_banner`,`standing_banner`]);function sa(e,t,n=!1,r=0){return n?e===`boss chamber`?8220772:r>=10?10721414:r>=5?9997692:9076850:e===`boss chamber`?9274745:e===`sewer tunnel`||e===`service passage`?9076076:[`arena floor`].includes(e)?13152644:[`kitchen`,`scullery`,`baths`,`forge`,`brewhouse`].includes(e)?9407104:e===`courtyard`||e===`garth`?la:e===`inn yard`?9801603:e===`ballroom`?12291420:e===`battlements`||e===`wall walk`||e===`roof`?11051670:e===`open to below`?-1:e===`roof terrace`?ua:e===`stands`?9075300:Kr.has(e)?da:t<0||`nave.sanctuary.crypt.ossuary.great hall.mess hall.dungeon.guardroom.armory.vault.chapel.oratory.courtroom.counting house.hall.entry hall.foyer.holding pen.beast pen.cell.warehouse floor.screens passage.mural passage.landing.entrance hall.gate passage.keep yard.well room.garrison.throne room.banqueting hall.royal chapel.stair hall.servants' hall.side chapel.shrine.chapter house.cloister walk.refectory.arcade.strongroom.warded vault.trophy hall`.split(`.`).includes(e)?10327948:11041359}var ca=e=>e===10327948||e===9407104||e===11051670||e===9801603,la=8362582,ua=12889480,da=11771256;function fa(e,t,n){if(n===la||n===ua||n===da)return[];if(!ca(n))return[1,2,3].map(n=>[[e,t+n/4],[e+1,t+n/4]]);let r=(t%2+2)%2;return[[[e,t+.5],[e+1,t+.5]],[[e+r*.5+.25,t],[e+r*.5+.25,t+.5]],[[e+(1-r)*.5+.25,t+.5],[e+(1-r)*.5+.25,t+1]]]}var pa=new Set([`battlements`,`wall walk`,`tower top`,Se]),ma=new Set([`hall`,`corridor`,`landing`,`screens passage`,`mural passage`,`entrance hall`,`cloister walk`,`keep yard`,`nave`,`minstrels' gallery`,`gate passage`,`courtyard`,`long gallery`,`stair hall`]);function ha(e){switch(e){case`bed`:case`cot`:return 8215100;case`rug`:return 9124410;case`hearth`:case`oven`:case`forge`:return 4867136;case`barrel`:case`keg_rack`:case`vat`:return 7227948;case`crate`:return 10123854;case`altar`:case`statue`:case`pillar`:case`sarcophagus`:case`bell`:return 12104358;case`cage`:return 5591629;case`trapdoor`:return 6965802;case`bath`:return 8364981;case`telescope`:case`anvil`:return 4146248;default:return 6176808}}function ga(e,t){let n=new Map;return e.rooms.forEach((e,r)=>{let i=t?.polys[r];e.kind===`open to below`&&e.shape===1&&i?.length&&n.set(r,i)}),n}function _a(e,t,n,r,i){let a=5660779,o=(e,t)=>{let n=n=>Math.max(0,Math.min(255,Math.round((e>>n&255)*t)));return n(16)<<16|n(8)<<8|n(0)},s=ga(t,i),c=s.size?xr(t,n,r):[];t.rooms.forEach((l,u)=>{if(l.kind!==`roof`)return;let[d,f,p,m,h]=[1/0,1/0,-1/0,-1/0,0];for(let e=0;e<n*r;e++){if(t.cells[e]!==u)continue;let[r,i]=[e%n,Math.floor(e/n)];[d,f,p,m,h]=[Math.min(d,r),Math.min(f,i),Math.max(p,r+1),Math.max(m,i+1),h+1]}if(!h)return;let g=[...s.keys()].find(e=>{let t=c[e];return!!t&&t[0]>d&&t[2]<p&&t[1]>f&&t[3]<m});if(g!==void 0&&i){let t=c[g],[n,r]=[(t[0]+t[2])/2,(t[1]+t[3])/2],s=Math.min(t[2]-t[0],t[3]-t[1])/2,l=i.real?Math.min(...i.parts.map(e=>K([n,r],e)))-q-.85:Math.min(p-n,n-d,m-r,r-f)-.15;if(l>s+.4){let t=(e,t)=>[n+e*Math.cos(t),r+e*Math.sin(t)];for(let n=0;n<64;n++){let[r,i]=[n/64*2*Math.PI,(n+1)/64*2*Math.PI];e.poly([...t(l,r),...t(l,i+.002),...t(s,i+.002),...t(s,r)]).fill(o(a,.975+.175*Math.sin((r+i)/2)))}for(let t=s+.45;t<l-.1;t+=.45)e.circle(n,r,t);e.stroke({width:.03,color:3093819,alpha:.35});for(let n=0;n<16;n++){let r=n/16*2*Math.PI;e.moveTo(...t(s,r)).lineTo(...t(l,r))}e.stroke({width:.04,color:9080985,alpha:.6}),e.circle(n,r,l).stroke({width:.06,color:N}),e.circle(n,r,s).stroke({width:.08,color:N});return}}if(h!==(p-d)*(m-f)){for(let i=0;i<n*r;i++)t.cells[i]===u&&e.rect(i%n,Math.floor(i/n),1,1);e.fill(a);return}let _=p-d>=m-f,v=(_?m-f:p-d)/2,[y,b]=_?[[d+v,(f+m)/2],[p-v,(f+m)/2]]:[[(d+p)/2,f+v],[(d+p)/2,m-v]],x=_?[[[d,f,p,f,...b,...y],1.15],[[d,m,p,m,...b,...y],.8],[[d,f,...y,d,m],1],[[p,f,...b,p,m],.9]]:[[[d,f,d,m,...b,...y],1.15],[[p,f,p,m,...b,...y],.8],[[d,f,...y,p,f],1],[[d,m,...b,p,m],.9]];for(let[t,n]of x)e.poly(t).fill(o(a,n));for(let t=.45;t<v-.1;t+=.45)e.rect(d+t,f+t,p-d-2*t,m-f-2*t);e.stroke({width:.03,color:3093819,alpha:.35}),e.moveTo(d,f).lineTo(...y).lineTo(d+(_?0:p-d),f+(_?m-f:0)),e.moveTo(p,m).lineTo(...b).lineTo(p-(_?0:p-d),m-(_?m-f:0)),e.stroke({width:.05,color:9080985,alpha:.8}),e.moveTo(...y).lineTo(...b).stroke({width:.09,color:10133672}),e.rect(d,f,p-d,m-f).stroke({width:.06,color:N})})}var va=new WeakMap;function ya(e,t,n,r,i){let a=ga(t,i),o=e=>{let[i,a]=[Math.floor(e[0]),Math.floor(e[1])];return i>=0&&a>=0&&i<n&&a<r?t.cells[a*n+i]:-1},s=e=>{let t=[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2],n=e.a[0]===e.b[0]?[.5,0]:[0,.5];return a.has(o([t[0]-n[0],t[1]-n[1]]))||a.has(o([t[0]+n[0],t[1]+n[1]]))},c=e=>{let n=[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2],r=e.a[0]===e.b[0]?[.5,0]:[0,.5];return[o([n[0]-r[0],n[1]-r[1]]),o([n[0]+r[0],n[1]+r[1]])].some(e=>e>=0&&t.rooms[e].kind===`garth`)},l=t.walls.filter(e=>e.rail&&!s(e)),u=l.filter(e=>!c(e)).map(e=>[e.a,e.b]),d=l.filter(c).map(e=>[e.a,e.b]);for(let[e,n]of a){let r=Yn(n);u.push(...Xn(r,n=>{let i=r[n],a=o([(i.a[0]+i.b[0])/2+i.out[0]*.35,(i.a[1]+i.b[1])/2+i.out[1]*.35]);return a>=0&&a!==e&&ma.has(t.rooms[a].kind)}))}let f=t=>{for(let n of t){e.moveTo(n[0][0],n[0][1]);for(let t of n.slice(1))e.lineTo(t[0],t[1])}},p=(t,n,r)=>{for(let i of t){let t=r,a=0;for(let r=0;r+1<i.length;r++){let[o,s]=[i[r],i[r+1]],c=Math.hypot(s[0]-o[0],s[1]-o[1]);for(;t<=a+c+1e-6;t+=1){let r=c>0?(t-a)/c:0;e.circle(o[0]+(s[0]-o[0])*r,o[1]+(s[1]-o[1])*r,n)}a+=c}}};d.length&&(f(d),e.stroke({width:.2,color:4867392,cap:`butt`,join:`round`}),f(d),e.stroke({width:.12,color:11840926,cap:`butt`,join:`round`}),p(d,.1,.5),e.fill(7235420),p(d,.055,.5),e.fill(14209212)),u.length&&(f(u),e.stroke({width:.16,color:4008471,cap:`round`,join:`round`}),f(u),e.stroke({width:.07,color:10779210,cap:`round`,join:`round`}),p(u,.09,0),e.fill(4008471))}function ba(e,t,n,r,i){let a=t.walls.filter(e=>e.arcade);if(!a.length)return;let o=(e,i)=>{let[a,o]=[Math.floor(e),Math.floor(i)];return a>=0&&o>=0&&a<n&&o<r?t.cells[o*n+a]:-1},s=[];for(let t of a){let n=Math.hypot(t.b[0]-t.a[0],t.b[1]-t.a[1]);if(n<1e-6)continue;let[r,a]=[(t.b[0]-t.a[0])/n,(t.b[1]-t.a[1])/n],c=[(t.a[0]+t.b[0])/2,(t.a[1]+t.b[1])/2],[l,u]=[-a,r];o(c[0]+l*.5,c[1]+u*.5)>=0&&([l,u]=[-l,-u]);let d=i?q:0,[f,p,m,h]=[t.a[0]+l*d,t.a[1]+u*d,t.b[0]+l*d,t.b[1]+u*d];e.moveTo(f,p).lineTo(m,h).stroke({width:.05,color:4867392,alpha:.45});let g=Math.max(1,Math.round(n/xa));for(let e=0;e<=g;e++)s.push([f+(m-f)*e/g,p+(h-p)*e/g])}let c=.2;for(let[t,n]of s)e.rect(t-c,n-c,2*c,2*c);e.fill(9406330).stroke({width:.06,color:N});for(let[t,n]of s)e.rect(t-c*.55,n-c*.55,1.1*c,1.1*c);e.fill(13222318)}var xa=2,Sa=9325624,Ca={"felt sanctuary":4610426,"felt hall":6251320},wa=12095560,Ta=9984580,Ea=10518618,Da=.36;function Oa(e,t,n,r,i=40){let a=[];for(let o=0;o<i;o++){let s=o/i*Math.PI*2,c=n*(1+.012*(Z(r,o%(i-1),5)-.5));a.push(e+Math.cos(s)*c,t+Math.sin(s)*c)}return a}function $(e,t,n,r,i){let[a,o]=[-n[1],n[0]],[s,c]=[r/2,i/2];return[e-n[0]*s-a*c,t-n[1]*s-o*c,e+n[0]*s-a*c,t+n[1]*s-o*c,e+n[0]*s+a*c,t+n[1]*s+o*c,e-n[0]*s+a*c,t-n[1]*s+o*c]}function ka(e,t,n,r=Sa){let[i,a]=t.c,o=Math.round(i*97+a*31),s=t.south;if(n){let[e,t]=[(n.a[0]+n.b[0])/2-i,(n.a[1]+n.b[1])/2-a],r=Math.hypot(e,t);r>.1&&(s=[e/r,t/r])}let c=[-s[1],s[0]],l=Math.max(.9,t.r-.6),u=.5,d=t.r-.75,f=$(i-s[0]*d,a-s[1]*d,c,Math.min(1.5,t.r),.7);e.poly(f).fill(6965812).stroke({width:Aa,color:N,join:`round`}),e.poly($(i-s[0]*d,a-s[1]*d,c,Math.min(1.5,t.r)-.16,.54)).stroke({width:.02,color:wa}),e.poly(Oa(i,a,l,o)).fill(wa).stroke({width:Aa,color:N,join:`round`}),e.poly(Oa(i,a,l-.09,o+1)).fill(r),e.poly(Oa(i,a,l-.16,o+2)).stroke({width:.018,color:5911588});let p=(l-.16+u)/2,m=Math.max(6,Math.round(Math.PI*2*p/.42));for(let t=0;t<m;t++){let n=t/m*Math.PI*2,[r,o]=[i+Math.cos(n)*p,a+Math.sin(n)*p],[s,c]=[Math.cos(n),Math.sin(n)],[l,u]=[.11,.07];e.poly([r-s*l,o-c*l,r-c*u,o+s*u,r+s*l,o+c*l,r+c*u,o-s*u]).fill(wa),e.circle(r,o,.022).fill(5911588)}e.poly(Oa(i,a,u,o+3,24)).fill(9402974).stroke({width:Aa,color:N,join:`round`});let h=t.r-.3,g=l-.25,_=h-g;if(_>.2){let t=(h+g)/2;e.poly($(i+s[0]*t,a+s[1]*t,s,_,.72)).fill(5664874).stroke({width:Aa,color:N,join:`round`}),e.poly($(i+s[0]*t,a+s[1]*t,s,_-.14,.56)).stroke({width:.02,color:wa})}}var Aa=.025;function ja(e,t,n){let[[r,i],a]=[t.c,t.r],o=Math.PI*2,s=n.map(e=>{let t=e.outer??[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2];return{at:Math.atan2(t[1]-i,t[0]-r),half:Math.asin(Math.min(.95,Math.min(Math.hypot(e.b[0]-e.a[0],e.b[1]-e.a[1]),.95))/2/a)}}),c=[];if(!s.length)c.push([0,o]);else{let e=s.map(e=>[e.at-e.half,e.at+e.half]).sort((e,t)=>e[0]-t[0]);for(let t=0;t<e.length;t++){let n=t+1<e.length?e[t+1][0]:e[0][0]+o;n-e[t][1]>.05&&c.push([e[t][1],n])}}let l=(e,t)=>[r+Math.cos(e)*t,i+Math.sin(e)*t],u=Da/2-.05,d=.1;for(let[t,n]of c){let r=n-t>=o-1e-9,i=Math.max(2,Math.ceil((n-t)*a/.2)),s=Array.from({length:i+1},(e,r)=>t+(n-t)*r/i),c=e=>s.flatMap(t=>l(t,e)),f=(e,t)=>r?null:[...c(t),...s.slice().reverse().flatMap(t=>l(t,e))];for(let[t,n,r]of[[a-Da/2,a+Da/2,N],[a-u,a+u,Ea]]){let i=f(t,n);i?e.poly(i).fill(r):e.poly(c(n)).fill(r).poly(c(t)).cut()}for(let t=0;t<i;t++){let[n,r,i,o]=[l(s[t],a-d),l(s[t],a+d),l(s[t+1],a-d),l(s[t+1],a+d)];e.moveTo(n[0],n[1]).lineTo(o[0],o[1]),e.moveTo(r[0],r[1]).lineTo(i[0],i[1])}e.stroke({width:.022,color:6176808,cap:`round`})}}function Ma(e,t,n,r){let i=r.c,a=Math.hypot(t.b[0]-t.a[0],t.b[1]-t.a[1])||1,o=[(t.b[0]-t.a[0])/a,(t.b[1]-t.a[1])/a],[s,c]=[(t.a[0]+t.b[0])/2-i[0],(t.a[1]+t.b[1])/2-i[1]],l=r.r/(Math.hypot(s,c)||1),[u,d]=[i[0]+s*l,i[1]+c*l],f=Math.min(a,.95),[p,m,h,g]=[u-o[0]*f/2,d-o[1]*f/2,u+o[0]*f/2,d+o[1]*f/2],_=(i[0]-u)*-o[1]+(i[1]-d)*o[0]>=0?1:-1,v=[-o[1]*_,o[0]*_];for(let[t,n]of[[p,m],[h,g]])e.poly($(t,n,o,.12,.39999999999999997)).fill(7227948).stroke({width:Aa,color:N,join:`round`});let y=(f-.12)/2,b=(t,n,r,i)=>{e.poly($(t,n,r,i,.16)).fill(Ta).stroke({width:Aa,color:N,join:`round`}),e.poly($(t,n,r,i-.08,.08)).stroke({width:.016,color:wa});let[a,o]=[.06,.04];e.poly([t-r[0]*a,n-r[1]*a,t-r[1]*o,n+r[0]*o,t+r[0]*a,n+r[1]*a,t+r[1]*o,n-r[0]*o]).fill(wa)};if(!n){for(let e of[-1,1])b(u+o[0]*e*y*.5,d+o[1]*e*y*.5,o,y);return}for(let[e,t]of[[p+o[0]*.06,m+o[1]*.06],[h-o[0]*.06,g-o[1]*.06]])b(e+v[0]*y*.5,t+v[1]*y*.5,v,y)}var Na=1.75,Pa=1,Fa=.85,Ia=1,La=new WeakMap,Ra=null,za=()=>Ra??=new b({attributes:{aPosition:{buffer:new Float32Array([0,0,1,0,1,1,0,1]),format:`float32x2`},aUV:{buffer:new Float32Array([0,0,1,0,1,1,0,1]),format:`float32x2`}},indexBuffer:new Uint32Array([0,1,2,0,2,3])});function Ba(e){let t=new $a({id:`u:warmup`,settlement:0,building:0,name:null,function:`dungeon`,origin:[0,0],axis:[1,0],across:[0,1],nx:2,ny:2,levels:[{z:-1,name:``,elevation_ft:0,cells:[0,0,-1,0],rooms:[{kind:`chamber`,squares:3,raise_ft:0,center:[1,1]}],walls:[],doors:[],windows:[],furniture:[],roof:!1,has_stairs:!1,natural:!1}],entry_level:0,stairs:[0,0,0,0]},0),n=ue.create({width:4,height:4});e.render({container:t.container,target:n}),n.destroy(!0),t.destroy()}var Va=e=>e.charAt(0).toUpperCase()+e.slice(1),Ha=(e,t)=>{let n=t?/^(.*\b(?:up|down)) to the .+$/.exec(e):null;return n?`${Va(n[1])} to ${t}`:Va(e)+(t?` · ${t}`:``)},Ua=e=>(Math.round(e)%24576+24576)%24576,Wa=!0;function Ga(e){Wa=e}var Ka=2.4,qa=1.7,Ja=1.35,Ya=11568724,Xa=10,Za={treasure:11041308,"read-aloud":3104660,trap:10566182,secret:6176646,monster:4612394};function Qa(e){let t=e&&Za[e]||7031340,n=new P().circle(0,0,11.5).fill({color:2760468,alpha:.55}).circle(0,0,Xa).fill(16182480).stroke({width:2,color:t}),r={width:1.4,color:t,cap:`round`,join:`round`};switch(e){case`treasure`:n.roundRect(-5,-1.5,10,6,1).fill(15255146).stroke(r),n.moveTo(-5,-1.5).quadraticCurveTo(0,-6.5,5,-1.5).stroke(r),n.moveTo(-5,1).lineTo(5,1).stroke(r),n.rect(-1,.2,2,2.4).fill(t);break;case`read-aloud`:n.ellipse(0,-1.2,5.6,4).stroke(r),n.moveTo(-2.6,2.3).lineTo(-3.8,5.4).lineTo(0,2.7).stroke(r),n.moveTo(-2.8,-2.2).lineTo(2.8,-2.2).moveTo(-2.8,-.2).lineTo(1.6,-.2).stroke({...r,width:1.1});break;case`trap`:n.moveTo(-5.5,3.6).lineTo(5.5,3.6).stroke(r),n.moveTo(-5,3.6).lineTo(-3.7,-1.8).lineTo(-2.2,1.6).lineTo(-.8,-3.6).lineTo(.8,1.6).lineTo(2.2,-1.8).lineTo(3.7,1.6).lineTo(5,3.6).stroke(r);break;case`secret`:n.circle(0,-1.8,2.4).fill(t),n.poly([-1.3,-.6,1.3,-.6,2.2,4.6,-2.2,4.6]).fill(t);break;case`monster`:n.moveTo(-4.4,1.2).quadraticCurveTo(-5.4,-5.6,0,-5.8).quadraticCurveTo(5.4,-5.6,4.4,1.2).lineTo(2.6,2.4).lineTo(2.6,4.8).lineTo(-2.6,4.8).lineTo(-2.6,2.4).closePath().fill(15984585).stroke(r),n.circle(-1.9,-1.4,1.3).circle(1.9,-1.4,1.3).fill(t),n.moveTo(-.9,3).lineTo(-.9,4.8).moveTo(.9,3).lineTo(.9,4.8).stroke({...r,width:1});break;default:n.moveTo(-3.6,-5).lineTo(1.6,-5).lineTo(3.8,-2.8).lineTo(3.8,5).lineTo(-3.6,5).closePath().fill(16775920).stroke(r),n.moveTo(-1.8,-1.6).lineTo(2,-1.6).moveTo(-1.8,.8).lineTo(2,.8).moveTo(-1.8,3).lineTo(.8,3).stroke({...r,width:1})}let i=new k;return i.addChild(n),i}var $a=class e{interior;sea;peek;ghost;style;tile;static names={};static ways={};static renderer=null;static picked=null;static pickedAt=0;static notes=[];static notesOn=!0;container=new k;pickedRoom=new P;pickedName=new k;pickedSpot=null;pickedRi=-1;dim=new o(l.WHITE);plan=new k;ghostLayer=new k;ghostKey=``;labels=new k;level=0;labelSpots=[];marks=new k;npcs=[];markSpots=[];cam=null;under=null;fields=new Map;runs=new WeakMap;rough=new Map;anchors=new WeakMap;light=[-Math.SQRT1_2,-Math.SQRT1_2];warming=null;constructor(t,n,r=!1,i=!1,a={player:!1,doors:null},o=!1,s,c){this.interior=t,this.sea=n,this.peek=r,this.ghost=i,this.style=a,this.tile=o;for(let[e,t]of s??[])this.fields.set(e,t);this.dim.tint=1314828,this.dim.alpha=this.underground?ra:.55,this.dim.visible=!r&&!i&&!o,this.labels.visible=!r&&!i,this.marks.visible=!r&&!i&&!a.player,this.container.addChild(this.dim,this.plan,this.pickedRoom,this.ghostLayer,this.labels,this.pickedName,this.marks),this.level=Math.max(0,Math.min(t.levels.length-1,c??t.entry_level));let[l,u]=[-Math.SQRT1_2,-Math.SQRT1_2],d=Math.round(Math.atan2(l*t.across[0]+u*t.across[1],l*t.axis[0]+u*t.axis[1])/(Math.PI/4))*(Math.PI/4);this.light=[Math.cos(d),Math.sin(d)],this.draw(),!r&&!i&&!o&&t.levels.length>1&&e.renderer&&(this.warming=this.warmLevels())}*warmLevels(){let t=U.for(e.renderer),n=this.level,r=this.interior.levels.map((e,t)=>t).filter(e=>e!==n).sort((e,t)=>Math.abs(e-n)-Math.abs(t-n)||e-t);for(let e of r){for(;this.underground&&!this.fields.has(e);)yield!1;for(let n of this.interior.levels[e].furniture){if(n.sprite||!this.drawnAsItem(n))continue;let r=this.level;this.level=e;try{t.get(this.itemKey(n),e=>this.drawItem(e,n),[n.x,n.y])}finally{this.level=r}yield!0}}}drawnAsItem(e){return this.underground&&(e.kind===`sewage`||e.kind===`lava`)||Yi.has(e.kind)||Xi.has(e.kind)||e.kind===`platform`||e.kind===`balcony`||this.style.player&&e.kind===`key`||this.tile&&e.kind===`continues`?!1:!(this.style.player&&Pt(e.hazard))}get underground(){return/^[uwk]:/.test(this.interior.id)}linkAt(e,t){let[n,r]=this.toGrid(e,t),[i,a]=[Math.floor(n),Math.floor(r)];return this.interior.levels[this.level].links?.find(e=>e.x===i&&e.y===a)?.to??null}roomAt(e,t){let n=this.interior,[r,i]=this.toGrid(e,t).map(Math.floor),a=r>=0&&i>=0&&r<n.nx&&i<n.ny?n.levels[this.level]?.cells[i*n.nx+r]??-1:-1;return a>=0?`r:${n.id}:${this.level}:${a}`:null}noteMoves(e,t){let n=this.roomAt(e,t);if(!n)return[];let r=Number(n.slice(n.lastIndexOf(`:`)+1)),i=this.toGrid(e,t),a=[Math.round(i[0]*100)/100,Math.round(i[1]*100)/100];return[{kind:`room`,label:`Notes on ${this.roomName(this.level,r)}`,id:n,at:[e,t]},{kind:`note`,label:`Add a note here`,site:this.interior.id,level:this.level,grid:a,at:[e,t]}]}movesAt(t,n){let r=this.interior,[i,a]=this.toGrid(t,n),[o,s]=[Math.floor(i),Math.floor(a)];if(o<0||s<0||o>=r.nx||s>=r.ny)return[];let c=r.levels[this.level],l=r.levels.length,u=[],d=this.toWorld(o+.5,s+.5),f=(e,t,n)=>({kind:`level`,label:`${Va(n??`go ${t}`)} to ${this.levelName(e)}`,level:e,from:r.id,at:d}),p=this.wayNear(c,i,a),m=p??ia(c,o,s),h=p?void 0:c.links?.find(e=>e.far&&e.x===o&&e.y===s);h?.far&&u.push({kind:`site`,label:this.linkText(h),to:h.to,from:r.id,at:d,far:h.far});let g=p?this.wayLink(c,p):h?void 0:c.links?.find(e=>e.x===o&&e.y===s),_=g&&e.siteName(g.to);g&&u.push({kind:`site`,label:Ha(m?.name??`follow the way through`,_),to:g.to,from:r.id,at:d}),m&&!g&&!h&&(m.kind===`exit`?u.push({kind:`surface`,label:`Up to the surface (${m.name.toLowerCase()})`,at:d}):m.kind===`up`&&this.level+1<l?u.push(f(this.level+1,`up`,m.name)):m.kind===`down`&&this.level>0&&u.push(f(this.level-1,`down`,m.name)));let[v,y,b,x]=r.stairs;if(!this.underground&&c.has_stairs&&o>=v&&o<v+b&&s>=y&&s<y+x&&(this.level+1<l&&r.levels[this.level+1].has_stairs&&u.push(f(this.level+1,`up`,`stairs up`)),this.level>0&&r.levels[this.level-1].has_stairs&&u.push(f(this.level-1,`down`,`stairs down`))),!this.underground&&this.level===r.entry_level){let e=c.doors.find(e=>{if(e.kind!==`front`&&e.kind!==`back`&&e.kind!==`arch`)return!1;let[t,n]=[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2];return Math.abs(i-t)<.9&&Math.abs(a-n)<.9});e&&u.push({kind:`surface`,label:e.kind===`arch`?`Leave by the carriage arch`:`Leave by the ${e.kind} door`,at:this.toWorld(...this.outside(e))})}return u}outside(e){let[t,n]=[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2],[r,i]=e.a[0]===e.b[0]?[1,0]:[0,1],a=this.interior.levels[this.level],o=((e,t)=>{let[n,r]=[Math.floor(e),Math.floor(t)];return n>=0&&r>=0&&n<this.interior.nx&&r<this.interior.ny&&a.cells[r*this.interior.nx+n]>=0})(t+r*.5,n+i*.5)?-1:1;return[t+r*.5*o,n+i*.5*o]}linkTo(e){for(let t=0;t<this.interior.levels.length;t++){let n=this.interior.levels[t].links?.find(t=>t.to===e);if(n)return{level:t,x:n.x,y:n.y}}return null}get sewer(){return this.interior.id.startsWith(`w:`)}get currentLevel(){return this.level}setLevel(e){let t=this.interior.levels.length;this.level=Math.max(0,Math.min(t-1,e)),this.draw()}prefetch(e){if(this.underground)for(let t=0;t<this.interior.levels.length;t++)this.fields.has(t)||e(this.interior,t).then(e=>{!this.dead&&!this.fields.has(t)&&this.fields.set(t,e)})}redraw(){this.draw()}update(t){this.cam=t;let n=this.interior,[r,i]=t.worldToScreen(n.origin[0],n.origin[1]),a=5*t.ppf;for(let e of[this.plan,this.pickedRoom,this.ghostLayer])e.position.set(r,i),e.rotation=Math.atan2(n.axis[1],n.axis[0]),e.scale.set(a);if(this.under&&(this.under.uniforms.uniforms.uTime=performance.now()/1e3,this.under.uniforms.uniforms.uGrid=+!!Wa),this.warming){let t=performance.now(),n=!1;for(;performance.now()-t<1.5;){let e=this.warming.next();if(e.done){this.warming=null;break}if(!e.value)break;n=!0}n&&U.for(e.renderer).flush()}let o=Math.hypot(t.width,t.height);this.dim.position.set((t.width-o)/2,(t.height-o)/2),this.dim.width=this.dim.height=o;for(let e of this.labelSpots){e.text.rotation=-t.turn;let n=this.toWorld(e.x,e.y),[r,i]=t.worldToScreen(n[0],n[1]);e.text.position.set(r,i),e.text.visible=e.min*a>70&&e.ri!==this.pickedRi}if(this.placeWays(t,a),this.pickedSpot){this.pickedName.position.set(...t.worldToScreen(...this.toWorld(...this.pickedSpot))),this.pickedName.rotation=-t.turn,!e.pickedAt&&a>=12&&(e.pickedAt=performance.now());let n=e.pickedAt?(performance.now()-e.pickedAt)/1e3:Ka;this.pickedRoom.alpha=n<Ka?.45+.55*Math.cos(n*Math.PI*2/1.6)**2:1}this.placeMarks(t)}placeMarks(e){for(let t of this.markSpots){let n=this.toWorld(t.x,t.y);t.node.position.set(...e.worldToScreen(n[0],n[1])),t.node.rotation=-e.turn}}setNpcs(e){JSON.stringify(e)!==JSON.stringify(this.npcs)&&(this.npcs=e,this.drawMarks())}npcSpot(e){if(e.at)return this.toGrid(e.at[0],e.at[1]);let t=this.interior.levels[e.level],n=t?.rooms.reduce((e,t)=>t.squares>e.squares&&t.kind!==`floor`?t:e,t.rooms[0]);return n?[n.center[0],n.center[1]+1.2]:[this.interior.nx/2,this.interior.ny/2]}drawMarks(){if(this.marks.removeChildren().forEach(e=>e.destroy({children:!0})),this.markSpots=[],!this.marks.visible)return;let t=new V({fontFamily:[`Palatino Linotype`,`Georgia`,`serif`],fontSize:12,fontWeight:`bold`,fill:`#fde68a`,stroke:{color:`#2a1f14`,width:3,join:`round`}}),n=this.npcs.filter(e=>e.level===this.level),r=new Map;for(let e of n){let[n,i]=this.npcSpot(e),a=`${Math.round(n)},${Math.round(i)}`,o=r.get(a)??0;r.set(a,o+1);let s=new k,c=new P().circle(0,0,7).fill(11817737).stroke({width:2,color:16639626}).moveTo(-2.5,-1).lineTo(0,-4).lineTo(2.5,-1).stroke({width:1.5,color:16639626}),l=new wt({text:e.name,style:t,anchor:{x:.5,y:0},resolution:2});l.position.set(0,8),s.addChild(c,l),this.marks.addChild(s),this.markSpots.push({node:s,x:n+o*1.2,y:i})}if(e.notesOn)for(let t of e.notes){if(t.site!==this.interior.id||t.level!==this.level)continue;let e=Qa(t.glyph);this.marks.addChild(e),this.markSpots.push({node:e,x:t.at[0],y:t.at[1],note:t.id})}}drawNotes(){this.drawMarks(),this.cam&&this.placeMarks(this.cam)}noteAt(e,t){if(!this.cam||!this.marks.visible||!this.container.visible)return null;let[n,r]=this.cam.worldToScreen(e,t),i=null,a=14;for(let e of this.markSpots){if(!e.note)continue;let t=Math.hypot(e.node.position.x-n,e.node.position.y-r);t<a&&([i,a]=[e.note,t])}return i}drawPicked(){let t=this.pickedRoom.clear();this.pickedName.removeChildren().forEach(e=>e.destroy()),[this.pickedSpot,this.pickedRi]=[null,-1];let n=/^r:(.+):(\d+):(\d+)$/.exec(e.picked??``);if(!n||n[1]!==this.interior.id||Number(n[2])!==this.level||this.peek||this.ghost||this.style.player)return;let r=Number(n[3]),{nx:i,ny:a}=this.interior,o=this.interior.levels[this.level],s=o?.cells;if(!s||!o.rooms[r])return;let c=(e,t)=>e>=0&&t>=0&&e<i&&t<a&&s[t*i+e]===r,l=null;for(let e=0;e<a;e++)for(let n=0;n<i;n++)c(n,e)&&(t.rect(n,e,1,1),l??=[n+.5,e+.5]);t.fill({color:16105546,alpha:.2});let u=()=>{for(let e=0;e<a;e++)for(let n=0;n<i;n++)c(n,e)&&(c(n,e-1)||t.moveTo(n,e).lineTo(n+1,e),c(n,e+1)||t.moveTo(n,e+1).lineTo(n+1,e+1),c(n-1,e)||t.moveTo(n,e).lineTo(n,e+1),c(n+1,e)||t.moveTo(n+1,e).lineTo(n+1,e+1))};if(u(),t.stroke({width:.42,color:2760468,alpha:.6,cap:`square`}),u(),t.stroke({width:.22,color:16172115,cap:`square`}),!l)return;let d=o.rooms[r].center;this.pickedSpot=c(Math.floor(d[0]),Math.floor(d[1]))?[d[0],d[1]]:l,this.pickedRi=r;let f=new V({fontFamily:[`Palatino Linotype`,`Georgia`,`serif`],fontSize:14,fontStyle:`italic`,fontWeight:`bold`,fill:`#fcd56a`,stroke:{color:`#2a1f14`,width:4,join:`round`}});this.pickedName.addChild(new wt({text:this.roomName(this.level,r),style:f,anchor:.5,resolution:2}))}toWorld(e,t){let n=this.interior;return[n.origin[0]+(n.axis[0]*e+n.across[0]*t)*5,n.origin[1]+(n.axis[1]*e+n.across[1]*t)*5]}toGrid(e,t){let n=this.interior,r=e-n.origin[0],i=t-n.origin[1];return[(r*n.axis[0]+i*n.axis[1])/5,(r*n.across[0]+i*n.across[1])/5]}inspect(e,t){let n=this.interior,r=n.levels[this.level];if(!r)return null;let[i,a]=this.toGrid(e,t),o=Math.floor(i),s=Math.floor(a);if(o<0||s<0||o>=n.nx||s>=n.ny)return null;let c=r.cells[s*n.nx+o];if(c<0||!r.rooms[c])return null;let l=o>=n.stairs[0]&&o<n.stairs[0]+n.stairs[2]&&s>=n.stairs[1]&&s<n.stairs[1]+n.stairs[3],u=ia(r,o,s),d=null;if(u){let e=r.links?.some(e=>e.x===o&&e.y===s),t=[u.blocks_move?`blocks movement`:`passable`,u.height_ft>=5?`${u.height_ft} ft tall`:``,u.hazard??``,e?`double-click to follow`:``].filter(Boolean).join(` · `);d={name:this.itemName(u),cover:ta[u.cover],notes:t}}else l&&(d={name:`stairs`,cover:ta[0],notes:`difficult terrain`});let f=Nt(r,n.nx,n.ny,o,s);f&&(d=d?{...d,cover:`${d.cover}; ${f.cover}`,notes:[d.notes,f.note].filter(Boolean).join(` · `)}:{name:`lip`,cover:f.cover,notes:f.note});let p=r.elevation_ft+Mt(r,n.nx,n.ny,o,s);return{elevationFt:Math.round(p-this.sea),tier:Math.floor((p-this.sea)/5),surface:`${this.roomName(this.level,c)} (${this.levelName(this.level)})`,object:d}}dead=!1;destroy(){this.dead=!0,this.container.destroy({children:!0}),this.freeUnder()}freeUnder(){this.under&&=(this.under.shader.destroy(),this.under.sources.forEach(e=>e.destroy()),null)}drawUnderground(e){let t=this.interior,{nx:n,ny:r}=t;this.freeUnder();let a=this.fields.get(this.level);a||this.fields.set(this.level,a=wi(t,this.level));let{w:o,h:s,pad:c,data:l,cells:u,liquidKind:d,shapes:p,shapeCount:m,cave:g,terrain:_,dist:v,lift:y}=a,b=oi.has(t.function),x=new i({resource:l,width:o,height:s,format:`rgba8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:b?`nearest`:`linear`,addressMode:`clamp-to-edge`}),S=new i({resource:u,width:n,height:r,format:`rgba8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:`nearest`}),C=new i({resource:p,width:Math.max(1,m),height:2,format:`rgba8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:`nearest`}),w=new i({resource:g??new Uint8Array(4),width:g?n*4:1,height:g?r*4:1,format:`rgba8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:`linear`,addressMode:`clamp-to-edge`}),T=new i({resource:_??new Uint8Array(4),width:_?n*4:1,height:_?r*4:1,format:`rgba8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:`linear`,addressMode:`clamp-to-edge`}),E=new i({resource:y?.data??new Uint8Array(4),width:y?n*y.res:1,height:y?r*y.res:1,format:`rgba8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:`linear`,addressMode:`clamp-to-edge`}),D=new i({resource:v,width:(n+8)*4,height:(r+8)*4,format:`r8unorm`,alphaMode:`no-premultiply-alpha`,scaleMode:`linear`,addressMode:`clamp-to-edge`}),O=this.tile?0:4,[k,A]=[-Math.SQRT1_2,-Math.SQRT1_2],j=Ln(t.function,t.theme,t.look),M=new h({uTheme:{value:j.style,type:`f32`},uStone:{value:new Float32Array(j.stone),type:`vec3<f32>`},uCoping:{value:new Float32Array(j.coping),type:`vec3<f32>`},uRock:{value:new Float32Array(j.rock),type:`vec3<f32>`},uLook:{value:j.feel,type:`f32`},uTint:{value:new Float32Array(j.tint),type:`vec3<f32>`},uGrow:{value:O,type:`f32`},uN:{value:new Float32Array([n,r]),type:`vec2<f32>`},uPad:{value:c,type:`f32`},uLight:{value:new Float32Array([k*t.axis[0]+A*t.axis[1],k*t.across[0]+A*t.across[1]]),type:`vec2<f32>`},uTime:{value:0,type:`f32`},uRough:{value:si(t.function),type:`f32`},uSquare:{value:+!!b,type:`f32`},uLiquid:{value:d,type:`f32`},uGhost:{value:+!!this.ghost,type:`f32`},uGridAlpha:{value:e.natural?.2:.28,type:`f32`},uGrid:{value:+!!Wa,type:`f32`},uOrigin:{value:new Float32Array(this.tile?[Ua(t.origin[0]/5),Ua(t.origin[1]/5)]:[0,0]),type:`vec2<f32>`},uShapeCount:{value:m,type:`f32`},uCaverns:{value:+!!g,type:`f32`},uHasTerrain:{value:+!!_,type:`f32`},uHasLift:{value:+!!y,type:`f32`}}),ee=new f({glProgram:na,resources:{uField:x,uCells:S,uShapes:C,uCave:w,uTerrain:T,uLift:E,uDist:D,uNoise:fn(),under:M}}),te=new ke({geometry:za(),shader:ee});te.position.set(-O,-O),te.scale.set(n+2*O,r+2*O),this.plan.addChild(te),this.under={uniforms:M,sources:[x,S,C,w,T,E,D],shader:ee,mesh:te,level:this.level}}draw(){let t=this.interior,n=t.levels[this.level],r=this.underground&&this.under&&this.under.level===this.level?this.under.mesh:null;for(let e of this.plan.removeChildren())e!==r&&e.destroy();if(r&&this.plan.addChild(r),this.labels.removeChildren().forEach(e=>e.destroy()),this.labelSpots=[],this.drawMarks(),this.drawPicked(),this.underground&&!r&&this.drawUnderground(n),this.ghost)return;let i=new P;this.plan.addChild(i);let{nx:a,ny:o}=t,s=e=>sa(n.rooms[e].kind,n.z,n.natural,n.rooms[e].raise_ft),c=this.underground?null:Jr(t,this.level);this.underground||this.drawBelow(i,c);let l=c?.real?c.parts:null,u=this.underground?null:qr(t,this.level);if(c){let e=Ca[n.rooms.find(e=>Kr.has(e.kind))?.kind??``]??9325624;Pr(i,c,n,s,fa,u?()=>ka(i,u,Yr(t,this.level)?.find((e,t)=>n.doors[t].kind===`front`)??null,e):void 0)}else if(!this.underground){for(let e=0;e<o;e++)for(let t=0;t<a;t++){let r=n.cells[e*a+t];r>=0&&s(r)>=0&&i.rect(t,e,1,1).fill(s(r))}for(let e=0;e<o;e++)for(let t=0;t<a;t++){let r=n.cells[e*a+t];if(r>=0&&s(r)>=0)for(let[n,a]of fa(t,e,s(r)))i.moveTo(n[0],n[1]).lineTo(a[0],a[1])}i.stroke({width:.03,color:0,alpha:.25});for(let e=0;e<o;e++)for(let t=0;t<a;t++){let r=n.cells[e*a+t];r>=0&&s(r)>=0&&i.rect(t,e,1,1)}i.stroke({width:.04,color:0,alpha:.35})}if(!this.underground){for(let e=0;e<o;e++)for(let t=0;t<a;t++){let r=n.cells[e*a+t];if(!(r<0||n.rooms[r].raise_ft<=0))for(let[s,c]of[[1,0],[-1,0],[0,1],[0,-1]]){let l=t+s>=0&&t+s<a&&e+c>=0&&e+c<o?n.cells[(e+c)*a+t+s]:-1;if(l>=0&&n.rooms[l].raise_ft<n.rooms[r].raise_ft){let[n,r,a,o]=s===1?[t+1,e,t+1,e+1]:s===-1?[t,e,t,e+1]:c===1?[t,e+1,t+1,e+1]:[t,e,t+1,e];i.moveTo(n,r).lineTo(a,o)}}}i.stroke({width:.18,color:N}),_a(i,n,a,o,c)}let[d,f,p,m]=t.stairs,h=!!l&&[[d,f],[d+p,f],[d,f+m],[d+p,f+m]].some(e=>Qn(e,l)<.224),g=n.rooms.some(e=>e.kind===`roof terrace`)&&t.levels.filter(e=>e.z>=0).length<=2,_=e=>{if(g){let t=p>=m;e.rect(d+.08,f+.08,p-.16,m-.16).fill(6176808).stroke({width:.06,color:N}),e.rect(d+.18,f+.18,p-.36,m-.36).fill(2365970);let[n,r]=t?[f+.34,f+m-.34]:[d+.34,d+p-.34];for(let i of[n,r])t?e.moveTo(d+.2,i).lineTo(d+p-.2,i):e.moveTo(i,f+.2).lineTo(i,f+m-.2);e.stroke({width:.06,color:10253392});let i=Math.max(2,Math.round(Math.max(p,m)*2.5));for(let a=1;a<i;a++){let o=(t?d+.2:f+.2)+a/i*((t?p:m)-.4);t?e.moveTo(o,n).lineTo(o,r):e.moveTo(n,o).lineTo(r,o)}e.stroke({width:.045,color:8806460});return}e.rect(d,f,p,m).fill(8022616);let n=p>=m,r=Math.max(p,m)*3;for(let t=1;t<r;t++){let i=t/r;n?e.moveTo(d+i*p,f).lineTo(d+i*p,f+m):e.moveTo(d,f+i*m).lineTo(d+p,f+i*m)}e.stroke({width:.05,color:N,alpha:.7}),e.rect(d,f,p,m).stroke({width:.08,color:N});let i=this.level+1<t.levels.length&&t.levels[this.level+1].has_stairs,a=this.level>0&&t.levels[this.level-1].has_stairs,o=t=>{let r=d+p/2,o=f+m/2,s=(n?p:m)*.35,[c,l]=n?[t,0]:[0,t],u=i&&a?t===1?.18:-.18:0,[h,g]=n?[0,u]:[u,0];e.moveTo(r-c*s+h,o-l*s+g).lineTo(r+c*s+h,o+l*s+g),e.moveTo(r+c*s+h,o+l*s+g).lineTo(r+c*(s-.25)-l*.18+h,o+l*(s-.25)+c*.18+g),e.moveTo(r+c*s+h,o+l*s+g).lineTo(r+c*(s-.25)+l*.18+h,o+l*(s-.25)-c*.18+g)};i&&o(1),a&&o(-1),e.stroke({width:.09,color:15918792})};n.has_stairs&&!h&&_(i);let v=e.renderer?U.for(e.renderer):null,y=new k;for(let e of this.underground?[Xi]:[Yi,Xi])for(let t of n.furniture)e.has(t.kind)&&this.drawTerrain(i,t);for(let e of[...n.furniture].sort((e,t)=>+be.has(t.kind)-be.has(e.kind))){if(!this.drawnAsItem(e))continue;if(e.sprite&&v){let t=this.pictureOf(e);t?y.addChild(t):this.drawItem(i,e);continue}if(!v){this.drawItem(i,e);continue}let t=v.get(this.itemKey(e),t=>this.drawItem(t,e),[e.x,e.y]);t&&y.addChild(this.itemSprite(e,t))}if(v){if(v.flush(),this.plan.addChild(y),l){let e=new P;for(let t of l)e.poly(t.flatMap(e=>e)).fill(16777215);this.plan.addChild(e),y.mask=e}i=new P,this.plan.addChild(i)}else y.destroy();let b=Yr(t,this.level),x=c&&n.z>=0?zr(c,n):[],S=null;if(this.underground){let e=Xr(n,a,o);for(let[t,n]of e)i.moveTo(t[0],t[1]).lineTo(n[0],n[1]);i.stroke({width:.3,color:7301729,cap:`square`});for(let[t,n]of e)i.moveTo(t[0],t[1]).lineTo(n[0],n[1]);if(i.stroke({width:.08,color:N,alpha:.6,cap:`square`}),oi.has(t.function)){if(n.rooms.some(e=>e.shape)){let{index:e}=Hr(n,a,o),t=(t,r)=>t>=0&&r>=0&&t<a&&r<o&&n.cells[r*a+t]>=0&&Ur(e[r*a+t]),r=(e,t)=>e<0||t<0||e>=a||t>=o||n.cells[t*a+e]<0;for(let e=0;e<=o;e++)for(let n=0;n<=a;n++)n<a&&(t(n,e-1)&&r(n,e)||r(n,e-1)&&t(n,e))&&i.moveTo(n,e).lineTo(n+1,e),e<o&&(t(n-1,e)&&r(n,e)||r(n-1,e)&&t(n,e))&&i.moveTo(n,e).lineTo(n,e+1)}else for(let e of n.walls)e.exterior&&i.moveTo(e.a[0],e.a[1]).lineTo(e.b[0],e.b[1]);i.stroke({width:.08,color:N,alpha:.85,cap:`square`,join:`miter`})}}else if(c&&b){Fr(i,c,N);let e=e=>this.style.player&&n.doors[e].kind===`secret`&&!!this.style.doors&&!this.style.doors(ye(t.id,this.level),e).found&&!this.style.doors(ye(t.id,this.level),e).open,r=b.map((t,n)=>e(n)?{...t,inner:void 0,outer:void 0,threshold:void 0}:t),l=Br(i,c,n,r,x,n.roof?.45:.28,u?0:.45,N);if(u&&ja(i,u,r.filter(e=>e.out&&e.outer)),S=c.real?l.filter(e=>{let[t,r]=[(e.a[0]+e.b[0])/2-e.out[0]*(q+.4),(e.a[1]+e.b[1])/2-e.out[1]*(q+.4)],[i,s]=[Math.floor(t),Math.floor(r)],c=i>=0&&s>=0&&i<a&&s<o?n.cells[s*a+i]:-1;return c>=0&&pa.has(n.rooms[c].kind)}):null,!c.real)for(let e of n.walls)e.exterior&&i.moveTo(e.a[0],e.a[1]).lineTo(e.b[0],e.b[1]).stroke({width:.45,color:N,cap:`square`});for(let e of r)e.threshold&&e.room!==void 0&&Vr(i,e.threshold,s(e.room),N);for(let e of x)e.splay&&Vr(i,e.splay,s(e.room),N)}else for(let e of n.walls)!e.rail&&!e.arcade&&i.moveTo(e.a[0],e.a[1]).lineTo(e.b[0],e.b[1]).stroke({width:e.exterior||n.roof?.45:.28,color:N,cap:`square`});if(this.underground||ya(i,n,a,o,c),this.underground||ba(i,n,a,o,!!c?.real),n.has_stairs&&h&&_(i),S?.length){let e=.75;for(let t of S){let n=Math.hypot(t.b[0]-t.a[0],t.b[1]-t.a[1]),[r,a]=[(t.b[0]-t.a[0])/n,(t.b[1]-t.a[1])/n];for(;e<n;e+=1){let[n,o]=[t.a[0]+r*e,t.a[1]+a*e],s=(e,[t,n])=>[-1,1].flatMap(r=>[-1,1].map(i=>[e[0]+t*.22*r-n*.22*i,e[1]+n*.22*r+t*.22*i])),l=(e,t)=>s(e,t).every(e=>Qn(e,c.parts)>=-q-.001),u=[Jn([n,o],c.parts),[r,a]].map(e=>({t:e,at:cr([n,o],e,.22,.22,c.parts)})).find(e=>Math.hypot(e.at[0]-n,e.at[1]-o)<=.2&&l(e.at,e.t));if(!u)continue;let[d,f]=u.at,[p,m]=u.t;i.poly([d-p*.22-m*.22,f-m*.22+p*.22,d+p*.22-m*.22,f+m*.22+p*.22,d+p*.22+m*.22,f+m*.22-p*.22,d-p*.22+m*.22,f-m*.22-p*.22])}e-=n}i.fill(7301729)}else if(n.roof&&!n.rooms.some(e=>e.kind===`roof terrace`)){for(let e of n.walls){if(!e.exterior)continue;let t=Math.abs(e.b[0]-e.a[0])+Math.abs(e.b[1]-e.a[1]),n=Math.sign(e.b[0]-e.a[0]),r=Math.sign(e.b[1]-e.a[1]);for(let a=.25;a<t;a+=1){let t=e.a[0]+n*a,o=e.a[1]+r*a;i.rect(t-.22+n*.12,o-.22+r*.12,.44,.44)}}i.fill(7301729)}if(this.underground)for(let e of n.furniture)e.kind!==`niche`||e.sprite||this.wallAnchor(e)||!this.rockBehind(e)||M().nicheMouth(i,e,this.light,this.propWall(e));if(n.z>=0&&c){for(let{at:[e,t],tangent:[n,r]}of x)i.poly([e-n*.3-r*.09,t-r*.3+n*.09,e+n*.3-r*.09,t+r*.3+n*.09,e+n*.3+r*.09,t+r*.3-n*.09,e-n*.3+r*.09,t-r*.3-n*.09]);x.length&&i.fill(12375776)}else if(n.z>=0){for(let[e,t,r,a]of n.windows)e===r?i.rect(e-.09,Math.min(t,a)+.2,.18,.6):i.rect(Math.min(e,r)+.2,t-.09,.6,.18);i.fill(12375776)}let C=this.style.doors,w=ye(t.id,this.level),T=n.doors.map((e,t)=>b?{...e,a:b[t].a,b:b[t].b}:e);T.forEach((e,t)=>{C&&e.kind!==`arch`&&this.drawPlayDoor(i,e,C(w,t),!!b?.[t].moved)});for(let[e,t]of(C?[]:T).entries()){if(t.kind===`arch`)continue;if(t.kind===`portcullis`){this.drawPortcullis(i,t,!1);continue}if(t.kind===`secret`){let[e,n]=t.a,[r,a]=t.b;for(let t=0;t<1;t+=.34)i.moveTo(e+(r-e)*t,n+(a-n)*t).lineTo(e+(r-e)*(t+.18),n+(a-n)*(t+.18));i.stroke({width:.14,color:9647082,alpha:.85});continue}if(t.kind===`front`&&u){Ma(i,t,!1,u);continue}let n=t.kind===`front`?5913114:8017200;if(b?.[e].moved){let e=Math.hypot(t.b[0]-t.a[0],t.b[1]-t.a[1]),[r,a]=[(t.b[0]-t.a[0])/e,(t.b[1]-t.a[1])/e],[o,s]=[-a,r],c=Math.atan2(s,o);i.moveTo(t.a[0],t.a[1]).lineTo(t.b[0],t.b[1]).stroke({width:.12,color:n}),i.moveTo(t.a[0],t.a[1]).lineTo(t.a[0]+(o*.9+r*.1)*e,t.a[1]+(s*.9+a*.1)*e).stroke({width:.1,color:n}),i.arc(t.a[0],t.a[1],.9*e,c-.2,c).stroke({width:.03,color:N,alpha:.5});continue}let[r,a]=t.a,[o,s]=t.b,c=r===o,l=c?r+.9:r+.1,d=c?a+.1:a+.9;i.moveTo(r,a).lineTo(o,s).stroke({width:.12,color:n}),i.moveTo(r,a).lineTo(l,d).stroke({width:.1,color:n}),i.arc(r,a,.9,c?0:Math.PI/2-.2,c?.2:Math.PI/2).stroke({width:.03,color:N,alpha:.5})}this.style.player||T.forEach((e,t)=>{if(!e.lock)return;let n=C?C(w,t):null;n?.open||Qi(i,(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2,qi(e.lock),!!n?.unlocked)});let E=new V({fontFamily:[`Palatino Linotype`,`Georgia`,`serif`],fontSize:12,fontStyle:`italic`,fill:`#f5ecd6`,stroke:{color:`#2a1f14`,width:3,join:`round`}});n.rooms.forEach((e,t)=>{let r=Math.floor(e.center[0]),i=Math.floor(e.center[1]);if(e.squares<4||n.cells[i*a+r]!==t||e.kind===`floor`||e.kind===`open to below`||e.kind===`roof`||e.kind===`junction`&&e.squares<=9||this.tile&&(e.kind===`sewer tunnel`||e.kind===`service passage`))return;let o=new wt({text:this.roomName(this.level,t),style:E,anchor:.5,resolution:2});this.labels.addChild(o);let s=Kr.has(e.kind)?.5*Math.sqrt(e.squares/Math.PI)+.5:0;this.labelSpots.push({text:o,x:e.center[0],y:e.center[1]+s,min:Math.sqrt(e.squares),ri:t})}),this.wayLabels(n)}drawBelow(t,n){let r=this.interior,i=r.levels[this.level],{nx:a,ny:o}=r,s=(e,t)=>e.cells[t]>=0&&e.rooms[e.cells[t]].kind===`open to below`,c=new Int16Array(a*o).fill(-1),l=!1;for(let e=0;e<a*o;e++)if(s(i,e)){for(let t=this.level-1;t>=0&&!(r.levels[t].cells[e]<0);t--)if(!s(r.levels[t],e)){c[e]=t,l=!0;break}}if(!l)return;let u=new k;this.plan.addChildAt(u,this.plan.getChildIndex(t)+1);let d=new P;u.addChild(d);let f=ga(i,n),p=e=>{for(let t of f.values())e.poly(t.flatMap(e=>e));for(let t=0;t<a*o;t++)c[t]>=0&&!f.has(i.cells[t])&&e.rect(t%a,Math.floor(t/a),1,1)},m=new P,h=e=>{let t=c[e];if(t<0)return null;let n=r.levels[t],i=n.cells[e],a=sa(n.rooms[i].kind,n.z,n.natural,n.rooms[i].raise_ft);return a<0?10327948:a},g=new Map;for(let e=0;e<a*o;e++){let t=h(e);if(t!==null){g.set(e,t);continue}if(!f.size)continue;let[n,r]=[e%a,Math.floor(e/a)];for(let[t,s]of[[1,0],[-1,0],[0,1],[0,-1],[1,1],[-1,-1],[1,-1],[-1,1]]){let c=(r+s)*a+n+t,l=n+t>=0&&r+s>=0&&n+t<a&&r+s<o&&f.has(i.cells[c])?h(c):null;if(l!==null){g.set(e,l);break}}}for(let[e,t]of g)d.rect(e%a,Math.floor(e/a),1,1).fill(t);p(m),m.fill(16777215);for(let[e,t]of g)for(let[n,r]of fa(e%a,Math.floor(e/a),t))d.moveTo(n[0],n[1]).lineTo(r[0],r[1]);d.stroke({width:.03,color:0,alpha:.25});let _=e.renderer?U.for(e.renderer):null,v=new k,y=new Set;for(let e=0;e<a*o;e++)c[e]>=0&&y.add(c[e]);let b=this.level;for(let e of y){this.level=e;for(let t of r.levels[e].furniture){let n=!1;for(let r=t.y;r<t.y+t.h&&!n;r++)for(let i=t.x;i<t.x+t.w&&!n;i++)n=i<a&&r<o&&c[r*a+i]===e;if(!n||this.style.player&&(Pt(t.hazard)||t.kind===`key`))continue;if(!_||t.sprite){this.drawItem(d,t);continue}let r=_.get(this.itemKey(t),e=>this.drawItem(e,t),[t.x,t.y]);r&&v.addChild(this.itemSprite(t,r))}}this.level=b,_?.flush(),u.addChild(v),u.addChild(m),u.mask=m;let x=new P;p(x),x.fill({color:1709586,alpha:.42}),u.addChild(x)}drawPlayDoor(e,t,n,r=!1){if(t.kind===`portcullis`)return this.drawPortcullis(e,t,!!n.open);let i=t.kind===`front`?qr(this.interior,this.level):null;if(i)return Ma(e,t,!!n.open,i);let[a,o]=t.a,[s,c]=t.b,l=t.kind===`secret`;if(l&&!n.found&&!n.open){if(this.style.player){if(r)return;this.underground?(e.moveTo(a,o).lineTo(s,c).stroke({width:.3,color:7301729,cap:`square`}),e.moveTo(a,o).lineTo(s,c).stroke({width:.08,color:N,alpha:.6,cap:`square`})):e.moveTo(a,o).lineTo(s,c).stroke({width:.28,color:N,cap:`square`});return}for(let t=0;t<1;t+=.34)e.moveTo(a+(s-a)*t,o+(c-o)*t).lineTo(a+(s-a)*(t+.18),o+(c-o)*(t+.18));e.stroke({width:.14,color:9647082,alpha:.85});return}let u=l?7032698:t.kind===`front`?5913114:8017200;if(!n.open){e.moveTo(a,o).lineTo(s,c).stroke({width:.26,color:N,cap:`butt`}),e.moveTo(a,o).lineTo(s,c).stroke({width:.16,color:u,cap:`butt`});return}let d=Math.atan2(c-o,s-a),f=d-Math.PI/2,p=Math.hypot(s-a,c-o)*.95;e.moveTo(a,o).lineTo(a+Math.cos(f)*p,o+Math.sin(f)*p).stroke({width:.12,color:u}),e.arc(a,o,p,f,d).stroke({width:.03,color:N,alpha:.5})}drawPortcullis(e,t,n){let[r,i]=t.a,a=Math.hypot(t.b[0]-r,t.b[1]-i);if(a<1e-6)return;let[o,s]=[(t.b[0]-r)/a,(t.b[1]-i)/a],c=-s*this.light[0]+o*this.light[1]>0?-1:1,[l,u]=[-s*c,o*c],d=(e,t)=>[r+o*e+l*t,i+s*e+u*t],f=(e,t,n,r)=>[...d(e,n),...d(t,n),...d(t,r),...d(e,r)],p=Math.max(4,Math.round(a*5)),m=Array.from({length:p},(e,t)=>.06+(a-.12)*(t+.5)/p);if(n){let t=f(.1,a-.1,-.045,.045);e.poly(t).fill(788743).stroke({width:.03,color:N,join:`miter`});let n=Math.max(3,Math.min(4,Math.round(a*3.2))),r=Array.from({length:n},(e,t)=>.2+(a-.4)*(t+.5)/n);for(let t of r)e.moveTo(...d(t,-.035)).lineTo(...d(t,.035));e.stroke({width:.05,color:Q[1],cap:`butt`});for(let[t,n]of[[-.07,.07],[a-.07,a+.07]])e.poly(f(t,n,-.17,.17)).fill(10787978).stroke({width:.045,color:N,join:`miter`}),e.poly(f(t+.035,n-.035,-.12,.12)).fill(12761768);return}e.poly(f(-.06,.08,-.16,.16)).fill({color:N,alpha:.9}),e.poly(f(a-.08,a+.06,-.16,.16)).fill({color:N,alpha:.9}),e.poly(f(.04,a-.04,.08,.3)).fill({color:N,alpha:.2});let h=(t,n)=>{for(let t of m)e.moveTo(...d(t,-.21)).lineTo(...d(t,.18));e.stroke({width:t,color:n,cap:`butt`})},g=(t,n)=>{for(let t of[-.12,.08])e.moveTo(...d(0,t)).lineTo(...d(a,t));e.stroke({width:t,color:n,cap:`butt`})};h(.1,N),g(.1,N),h(.055,Q[1]),g(.058,Q[0]);for(let t of[-.12,.08])e.moveTo(...d(.03,t-.014)).lineTo(...d(a-.03,t-.014));e.stroke({width:.016,color:Q[2],alpha:.9});for(let t of m)e.moveTo(...d(t-.014,-.19)).lineTo(...d(t-.014,.16));e.stroke({width:.012,color:Q[2],alpha:.7});for(let t of m)e.poly([...d(t-.05,.17),...d(t+.05,.17),...d(t,.32)]);e.fill(Q[1]).stroke({width:.018,color:N,join:`miter`});for(let t of m)e.circle(...d(t,-.12),.022);e.fill(Q[2])}autoBack(e){let t={...e,back:void 0},n=this.sharedKind(e.kind);return n?ee.has(n)?Hi(this.backSide({...t,kind:n})):null:X.has(e.kind)?Hi(this.propWall(t)):Ii.has(e.kind)?Hi(this.backWall(t)):null}setGhost(t,n=`ok`){let r=t?`${this.itemKey(t)}|${t.x},${t.y}|${t.sprite??0}|${t.back??``}|${n}`:``;if(r===this.ghostKey||(this.ghostKey=r,this.ghostLayer.removeChildren().forEach(e=>e.destroy()),!t))return;let i=e.renderer?U.for(e.renderer):null,a=t.sprite&&i?this.pictureOf(t):null;if(t.sprite&&i&&!a&&(this.ghostKey=``),!a&&i){let e=i.get(this.itemKey(t),e=>this.drawItem(e,t),[t.x,t.y]);i.flush(),e&&(a=this.itemSprite(t,e))}if(!a){let e=new P;this.drawItem(e,t),a=e}a.tint=n===`bad`?16743018:n===`doorway`?16761962:16777215,a.alpha=n===`ok`?.85:.75,this.ghostLayer.addChild(a)}static thumbs(t,n){let r=new e({id:t?`u:thumbs`:`b:thumbs`,settlement:0,building:0,name:null,function:t?`dungeon`:`house`,origin:[0,0],axis:[1,0],across:[0,1],nx:1,ny:1,levels:[{z:-1,name:``,elevation_ft:0,cells:[0],rooms:[{kind:` `,squares:1,raise_ft:0,center:[.5,.5]}],walls:[],doors:[],windows:[],furniture:[],roof:!1,has_stairs:!1,natural:!1}],entry_level:0,stairs:[0,0,0,0]},0,!1,!0),i=e.renderer?U.for(e.renderer):null,a=[];for(let{kind:e,w:t,h:s,key:c}of n){let n={kind:e,name:e,x:0,y:0,w:t,h:s,cover:0,blocks_move:!1,height_ft:3,back:2},l=new k,u=i?.get(r.itemKey(n),e=>r.drawItem(e,n));if(u){let e=new o(u.texture);e.position.set(u.x,u.y),e.scale.set(1/64),l.addChild(e)}else{let e=new P;r.drawItem(e,n),l.addChild(e)}a.push({key:c??e,node:l})}return i?.flush(),r.destroy(),a}static sheet(t,n){let r=e.renderer;return r?hn(r,`${t?`u`:`b`}:${n.map(e=>`${e.kind}:${e.w}x${e.h}`).join(`|`)}`,()=>pn(r,e.thumbs(t,n))):null}itemName(e){return e.sprite&&this.interior.sprites?.[e.sprite-1]?.name||e.name}pictureOf(e){let t=this.interior.sprites?.[(e.sprite??0)-1]?.asset;if(!t)return null;let n=rn(t);if(!n){let e=nn(n=>{n===t&&(e(),this.dead||this.draw())});return null}let r=Vi(e)?e.back:0,[i,a]=r%2?[n.height,n.width]:[n.width,n.height],s=new o(n),c=Math.min(e.w/Math.max(1,i),e.h/Math.max(1,a));return s.anchor.set(.5),s.scale.set(c),s.rotation=r*Math.PI/2,s.position.set(e.x+e.w/2,e.y+e.h/2),s}sharedKind(e){return Wi(e,this.underground)}itemSprite(e,t){let n=new o(t.texture);n.scale.set(1/64);let r=this.wallAnchor(e);return r?(n.pivot.set((e.w/2-t.x)*64,-t.y*64),n.position.set(r.x,r.y),n.rotation=r.angle):n.position.set(e.x+t.x,e.y+t.y),n}wallAnchor(t){if(!X.has(t.kind)||!this.underground||!e.renderer||Vi(t))return null;let n=this.interior.levels[this.level];if(n.paths?.length)return this.lineAnchor(t,n);if(t.kind===`niche`||this.tile||oi.has(this.interior.function))return null;let r=this.roughAnchors(n),i=this.anchors.get(t);return i===void 0&&this.anchors.set(t,i=this.roughAnchor(t,n,r)),i}lineAnchor(e,t){let n=this.anchors.get(e);if(n!==void 0)return n;let{nx:r,ny:i}=this.interior,a=e.x<r&&e.y<i?t.cells[e.y*r+e.x]:-1,o=null;if(a>=0&&t.rooms[a].kind===`sewer tunnel`){let n=La.get(t);n||La.set(t,n=ui(t,r,i));let a=yi(t,r,i,n,e.x+e.w/2,e.y+e.h/2);a&&(o=this.turnedTo(a))}return this.anchors.set(e,o),o}turnedTo({x:e,y:t,dx:n,dy:r}){let i=Math.atan2(n,-r),a=Math.round(i/(Math.PI/8)),[o,s]=[Math.cos(a*Math.PI/8),Math.sin(a*Math.PI/8)],[c,l]=this.light;return{x:e,y:t,angle:i,step:(a%16+16)%16,light:[c*o+l*s,-c*s+l*o]}}roughAnchors(e){let t=this.rough.get(this.level);if(t&&t.n===e.furniture.length)return t;let n=this.interior,r=this.fields.get(this.level);r||this.fields.set(this.level,r=wi(n,this.level));let i=e=>X.has(e.kind)&&e.kind!==`niche`&&!Vi(e),a={n:e.furniture.length,dist:ki(r,n.nx,n.ny,si(n.function)),taken:e.furniture.filter(e=>X.has(e.kind)&&!i(e)).map(e=>[e.x+e.w/2,e.y+e.h/2]),solid:e.furniture.filter(e=>!X.has(e.kind)&&!Ri.has(e.kind)&&ai[e.kind]===void 0),doors:e.doors.map(e=>[(e.a[0]+e.b[0])/2,(e.a[1]+e.b[1])/2])};this.rough.set(this.level,a);for(let t of e.furniture){if(!i(t))continue;let n=this.roughAnchor(t,e,a);this.anchors.set(t,n),a.taken.push(n?[n.x,n.y]:[t.x+t.w/2,t.y+t.h/2])}return a}roughAnchor(e,t,{dist:n,taken:r,solid:i,doors:a}){let{nx:o,ny:s}=this.interior,c=(e,t)=>{let[r,i]=[Math.floor(e),Math.floor(t)];return r>=0&&i>=0&&r<o&&i<s&&n.shaped[i*o+r]!==0},[l,u]=[e.x+e.w/2,e.y+e.h/2];if(c(l,u))return null;let d=(e,t,n)=>t>=e.x&&n>=e.y&&t<=e.x+e.w&&n<=e.y+e.h,f=Pi(n,l,u,Na,Pa,(e,t,n,o)=>{if(c(e,t))return 2;let[s,l]=[e-n*.4,t-o*.4];return r.some(([n,r])=>Math.hypot(n-e,r-t)<Fa)||a.some(([n,r])=>Math.hypot(n-e,r-t)<Ia)||i.some(n=>d(n,e,t)||d(n,s,l))?1:0});return f&&this.turnedTo(f)}propWall(e){return this.underground?this.wallSide(e):this.backSide(e)}lookOf(e){return e.variant===void 0?oa.has(e.kind)&&!this.underground?re(this.interior.id):aa(e.x,e.y,ne(e.kind)):e.variant}columnTones(){return M().columnTones(Ln(this.interior.function,this.interior.theme).stone)}itemKey(e){if(e.kind===`link`)return`link:${e.variant??0}:${+!!this.linkUp(e)}:${this.light.map(e=>Math.round(e*100)).join(`,`)}`;if(this.isWay(e))return`way:${this.underground?`u`:`b`}:${e.kind}:${e.w}x${e.h}:${this.light.map(e=>Math.round(e*100)).join(`,`)}:${this.lookOf(e)}:q${this.wayTurn()}:${this.wayScale(e)}`;let t=`${e.kind}:${e.w}x${e.h}`,n=this.light.map(e=>Math.round(e*100)).join(`,`),r=this.sharedKind(e.kind);if(r){let i={...e,kind:r},a=ee.has(r)?this.backSide(i).join(`,`):``,o=this.ringFit(e),s=o?`:y${[o.c[0]-e.x,o.c[1]-e.y,o.r].map(e=>e.toFixed(2)).join(`,`)}`:``,c=this.underground&&e.kind===`pillar`?`:s${this.columnTones().join(`,`)}`:``;return`${this.underground?`u`:`b`}:${t}:${n}:${te.has(r)?this.lookOf(e):0}:${a}${s}${c}`}let i=this.wallAnchor(e),a=i?`t${i.step}`:X.has(e.kind)?this.propWall(e).join(`,`):Ii.has(e.kind)?this.backWall(e).join(`,`):``,o=Gi.has(e.kind)||ae.has(e.kind)||e.kind===`key`?this.lookOf(e):0,s=e.kind===`rail`||e.kind===`ore_cart`?`:r${this.railLinks(e)}`:``,c=e.kind===`niche`&&!i?`:k${this.rockBehind(e)}`:``,l=zi.has(e.kind)?this.runOf(e):null;return`u:${t}:${n}:${a}:${o}${s}${c}${l?`:d${l.dir.map(e=>Math.round(e*16)).join(`,`)}`+(e.kind===`grille`?`:${(l.at[0]-e.x).toFixed(2)},${(l.at[1]-e.y).toFixed(2)}:${l.span.join(`,`)}`:``):``}`}runDir(e){return this.runOf(e).dir}runOf(e){let t=this.runs.get(e);if(t)return t;let n=this.interior.levels[this.level].paths??[],[r,i]=[e.x+.5,e.y+.5],a=[1,0],o=[r,i],s=1/0;for(let[e,t,c,l]of n){let[n,u]=[c-e,l-t],d=Math.max(n*n+u*u,1e-9),f=Math.max(0,Math.min(1,((r-e)*n+(i-t)*u)/d)),p=Math.hypot(r-e-f*n,i-t-f*u);if(p<s){s=p;let r=(n*.958+u*.287<0?-1:1)/Math.sqrt(d);a=[n*r,u*r],o=[e+f*n,t+f*u]}}let[c,l]=[Math.round(a[0]*16)/16,Math.round(a[1]*16)/16],u=Math.hypot(c,l)||1,d=[c/u,l/u];o=[Math.round(o[0]*20)/20,Math.round(o[1]*20)/20];let f=[.6,.6];if(e.kind===`grille`&&n.length){let e=this.fields.get(this.level);e||this.fields.set(this.level,e=wi(this.interior,this.level));let t=e,n=(e,n)=>{let r=Math.max(0,Math.min(t.w-1,Math.floor((e+t.pad)*4))),i=Math.max(0,Math.min(t.h-1,Math.floor((n+t.pad)*4)));return t.data[(i*t.w+r)*4+1]>=128},r=e=>{let t=0;for(;t<2.5&&n(o[0]-d[1]*e*(t+.05),o[1]+d[0]*e*(t+.05));)t+=.05;return Math.max(.3,Math.round(t*10)/10)};f=[r(-1),r(1)]}let p={dir:d,at:o,span:f};return this.runs.set(e,p),p}railLinks(e){if(e.kind!==`rail`&&e.kind!==`ore_cart`)return 0;let{nx:t,ny:n}=this.interior,r=this.interior.levels[this.level],i=r?.furniture.includes(e)?r.furniture:this.interior.levels.find(t=>t.furniture.includes(e))?.furniture??[],a=va.get(i);if(!a||a.n!==i.length){let e=new Map;for(let n of i)(n.kind===`rail`||n.kind===`ore_cart`)&&n.w===1&&n.h===1&&e.set(n.y*t+n.x,n.kind);a={n:i.length,at:e},va.set(i,a)}let o=a.at,s=(e,r)=>e<0||r<0||e>=t||r>=n?void 0:o.get(r*t+e),c=[[0,-1],[1,0],[0,1],[-1,0]],l=(e,t)=>c.reduce((n,[r,i],a)=>s(e+r,t+i)===`rail`?n|1<<a:n,0),u=e=>!!(e&e-1);if(e.kind===`rail`){let t=l(e.x,e.y);return u(t)?t:c.reduce((t,[n,r],i)=>s(e.x+n,e.y+r)===`ore_cart`?t|1<<i:t,t)}return c.reduce((t,[n,r],i)=>s(e.x+n,e.y+r)===`rail`&&!u(l(e.x+n,e.y+r))?t|1<<i:t,0)}rockBehind(e){let[t,n]=this.wallSide(e);if(!t&&!n)return 0;let r=this.interior.levels[this.level],{nx:i,ny:a}=this.interior,o=0;for(;o<2;o++){let[s,c]=[e.x+t*(o+1),e.y+n*(o+1)];if(s>=0&&c>=0&&s<i&&c<a&&r.cells[c*i+s]>=0)break}return o}wallSide(e){let t=Vi(e);if(t)return t;let n=this.interior.levels[this.level],{nx:r,ny:i}=this.interior;for(let[t,a]of[[0,-1],[-1,0],[1,0],[0,1]]){let[o,s]=[e.x+t,e.y+a];if(o<0||s<0||o>=r||s>=i||n.cells[s*r+o]<0)return[t,a]}return[0,0]}backSide(e){let t=Vi(e);if(t)return t;let n=this.interior.levels[this.level],[r,i]=[e.x+e.w/2,e.y+e.h/2];if(e.kind===`booth_seat`){let t=n.furniture.find(t=>t.kind===`booth_table`&&Math.abs(t.x+t.w/2-r)+Math.abs(t.y+t.h/2-i)<=(t.w+e.w)/2+.01);if(t){let[e,n]=[t.x+t.w/2-r,t.y+t.h/2-i];return Math.abs(e)>Math.abs(n)?[-Math.sign(e),0]:[0,-Math.sign(n)]}let a=n.furniture.find(t=>t.kind===`booth_table`&&Math.abs(t.x+t.w/2-r)<=(t.w+e.w)/2+.01&&Math.abs(t.y+t.h/2-i)<=(t.h+e.h)/2+.01);if(a)return[-Math.sign(a.x+a.w/2-r),-Math.sign(a.y+a.h/2-i)]}if(e.kind===`pew`){let t=n.rooms[n.cells[e.y*this.interior.nx+e.x]],[a,o]=t?t.center:[r,i],s=null;for(let e of n.furniture)e.kind===`altar`&&(!s||Math.hypot(e.x-a,e.y-o)<Math.hypot(s.x-a,s.y-o))&&(s=e);let[c,l]=s?[s.x+s.w/2-a,s.y+s.h/2-o]:[0,0];return e.w>=e.h?[0,l>.5?-1:1]:[c>.5?-1:1,0]}return this.backWall(e,e.kind===`bed`||e.kind===`stable_stall`)}backWall(e,t=!1){let n=Vi(e);if(n)return n;let r=this.interior.levels[this.level],{nx:i,ny:a}=this.interior,o=t?e.h>e.w:e.w>e.h,s=t?e.w>e.h:e.h>e.w,c=o?[[0,-1],[0,1]]:s?[[-1,0],[1,0]]:[[0,-1],[-1,0],[1,0],[0,1]];for(let[t,n]of c){let o=t!==0,s=t<0?e.x:t>0?e.x+e.w:n<0?e.y:e.y+e.h,[c,l]=o?[e.y,e.y+e.h]:[e.x,e.x+e.w],u=r.walls.some(e=>{if(o?e.a[0]!==s||e.b[0]!==s:e.a[1]!==s||e.b[1]!==s)return!1;let[t,n]=o?[e.a[1],e.b[1]]:[e.a[0],e.b[0]];return Math.min(Math.max(t,n),l)-Math.max(Math.min(t,n),c)>.5}),d=!1;for(let s=c;s<l&&!d;s++){let[c,l]=o?[t<0?e.x-1:e.x+e.w,s]:[s,n<0?e.y-1:e.y+e.h];d=c<0||l<0||c>=i||l>=a||r.cells[l*i+c]<0}if(u||d)return[t,n]}return c[0]}drawTerrain(e,t){let{x:n,y:r,w:i,h:a,kind:o}=t;switch(o){case`water_shallow`:e.rect(n,r,i,a).fill({color:7321776,alpha:.55});for(let t=0;t<a;t++)for(let a=0;a<i;a++){let[i,o]=[n+a+.2+Z(n+a,r+t,1)*.5,r+t+.25+Z(n+a,r+t,2)*.5];e.moveTo(i,o).lineTo(i+.25,o)}e.stroke({width:.03,color:15005426,alpha:.6});break;case`water_deep`:e.rect(n,r,i,a).fill({color:1788536,alpha:.92});for(let t=0;t<a;t++)for(let a=0;a<i;a++){let[i,o]=[n+a+.2+Z(n+a,r+t,3)*.4,r+t+.3+Z(n+a,r+t,4)*.4];e.moveTo(i,o).lineTo(i+.3,o)}e.stroke({width:.03,color:5212856,alpha:.5});break;case`chasm`:e.rect(n,r,i,a).fill(394501),e.rect(n+.04,r+.04,i-.08,a-.08).stroke({width:.08,color:6972248});break;case`rubble_slope`:e.rect(n,r,i,a).fill({color:9078140,alpha:.38});for(let t=0;t<a;t++)for(let a=0;a<i;a++)for(let i=0;i<4;i++){let[o,s]=[n+a+.1+Z(n+a,r+t,10+i)*.8,r+t+.1+Z(n+a,r+t,20+i)*.8];e.circle(o,s,.04+Z(n+a,r+t,30+i)*.05).fill(i%2?6183507:11841445)}break;case`rope_bridge`:{let t=i>=a,[o,s]=t?[i,a]:[a,i],c=(e,i)=>t?[n+e,r+i]:[n+i,r+e],l=(e,t,n,r,i)=>[...c(e+i[0],t),...c(n+i[1],t),...c(n+i[2],r),...c(e+i[3],r)],[u,d]=[.1,s-.1],f=[[9069112,11569754],[8215100,11043418],[9660734,12358242],[8414794,11046250]];for(let t=0;t<Math.round(o*4);t++){let i=t/4+.02,a=(t+1)/4-.02,o=e=>(Z(n*7+t,r*13+t,e)-.5)*.02,c=[o(1),o(2),o(3),o(4)],[u,d]=f[Math.floor(Z(n+t,r-t,5)*f.length)],[p,m]=[.04+o(6),s-.04+o(7)];e.poly(l(i,p,a,m,c)).fill(d),e.poly(l(i+(a-i)*.3,p,a,m,c)).fill(u),e.poly(l(a-(a-i)*.16,p,a,m,c)).fill(6176804),e.poly(l(i,p,a,m,c)).stroke({width:.016,color:N,join:`round`})}let p=[];for(let e of[.08,o-.08])for(let t of[u,d])p.push(c(e,t));for(let[t,n]of p)e.circle(t+.04,n+.05,.11).fill({color:N,alpha:.3});for(let t of[u,d])e.moveTo(...c(.08,t)).lineTo(...c(o-.08,t));e.stroke({width:.06,color:N,cap:`round`});for(let t of[u,d])e.moveTo(...c(.08,t)).lineTo(...c(o-.08,t));e.stroke({width:.03,color:13214826,cap:`round`});for(let t of[u,d])for(let n=.2;n<o-.15;n+=.14)e.moveTo(...c(n-.03,t-.02)).lineTo(...c(n+.03,t+.02));e.stroke({width:.014,color:8018478});for(let[t,n]of p)e.circle(t,n,.1).fill(6965802).stroke({width:.03,color:N}),e.circle(t-.02,n-.02,.06).fill(10122312),e.circle(t-.02,n-.02,.025).fill(6176804);for(let t=1;t<o;t++)e.moveTo(...c(t,.04)).lineTo(...c(t,s-.04));for(let t=1;t<s;t++)e.moveTo(...c(.12,t)).lineTo(...c(o-.12,t));e.stroke({width:.02,color:N,alpha:.3});break}case`stone_bridge`:{let t=i>=a,[o,s]=t?[i,a]:[a,i],c=(e,i)=>t?[n+e,r+i]:[n+i,r+e],l=(e,t,n,r)=>[...c(e,t),...c(n,t),...c(n,r),...c(e,r)],u=Math.min(.34,s*.3);for(let t of[0,1])for(let n=0;n<5;n++){let r=.1+n*.1,[i,a]=t?[s,s+r+.08]:[-r-.08,0];e.poly(l(.06+n*.03,i,o-.06-n*.03,a)).fill({color:0,alpha:.16})}let d=Math.max(1,Math.round((s-2*u)/.5)),f=(s-2*u)/d,p=[9407103,9736068,9012344,9999242];for(let t=0;t<d;t++){let i=t%2?.45:0;for(let a=-i;a<o;a+=.9){let[i,s]=[Math.max(0,a)+.015,Math.min(o,a+.9)-.015];if(s-i<.1)continue;let[c,d]=[u+t*f,u+(t+1)*f];e.poly(l(i,c,s,d)).fill(p[Math.floor(Z(n*5+Math.round(a*4),r*3+t,41)*p.length)]),e.poly(l(i+.03,c+.03,s-.03,c+.09)).fill({color:11578016,alpha:.5}),e.poly(l(i,c,s,d)).stroke({width:.03,color:N,alpha:.85,join:`round`})}}for(let t of[0,1]){let i=t?s-u:0;e.poly(l(0,i,o,i+u)).fill(3091496).stroke({width:.05,color:N,join:`round`});let[a,c]=[i+u*.2,i+u*.8];for(let i=0;i<o;i+=.5){let[s,u]=[i+.025,Math.min(o,i+.5)-.025];e.poly(l(s,a,u,c)).fill(Z(n+Math.round(i*2),r+t,43)<.5?5788491:6446420).stroke({width:.025,color:N,alpha:.9,join:`round`}),e.poly(l(s+.02,a+.02,u-.02,a+.07)).fill({color:9275259,alpha:.6})}}for(let t=1;t<o;t++)e.moveTo(...c(t,u)).lineTo(...c(t,s-u));for(let t=1;t<s;t++)e.moveTo(...c(0,t)).lineTo(...c(o,t));e.stroke({width:.02,color:N,alpha:.25});break}case`stepping_stone`:for(let t=0;t<a;t++)for(let a=0;a<i;a++){let i=(n+a)*31+(r+t)*17,[o,s]=[n+a+.5+(Z(n+a,r+t,5)-.5)*.12,r+t+.5+(Z(n+a,r+t,6)-.5)*.12],c=.3+Z(n+a,r+t,8)*.05;e.poly(ea(o+.03,s+.04,c*1.22,i)).fill({color:1319978,alpha:.4}),e.poly(ea(o,s,c*1.32,i+1,12)).stroke({width:.018,color:14216424,alpha:.35});let l=ea(o,s,c,i);e.poly(l).fill(6117456).stroke({width:.035,color:N,join:`round`}),e.poly(ea(o-.035,s-.04,c*.78,i)).fill(9406846),e.poly(ea(o-.075,s-.085,c*.38,i+2,7)).fill({color:12104099,alpha:.9}),e.moveTo(o+c*.15,s-c*.1).lineTo(o+c*.35,s+c*.25).stroke({width:.015,color:4867647,cap:`round`})}for(let t=1;t<i;t++)e.moveTo(n+t,r).lineTo(n+t,r+a);for(let t=1;t<a;t++)e.moveTo(n,r+t).lineTo(n+i,r+t);(i>1||a>1)&&e.stroke({width:.02,color:N,alpha:.3})}if(o.startsWith(`water`)||o===`chasm`){for(let t=1;t<i;t++)e.moveTo(n+t,r).lineTo(n+t,r+a);for(let t=1;t<a;t++)e.moveTo(n,r+t).lineTo(n+i,r+t);e.stroke({width:.02,color:0,alpha:.18})}}ringFit(e){if(this.underground||!ie.has(e.kind))return;let t=qr(this.interior,this.level);return t?{c:t.c,r:t.r-Da/2}:void 0}drawItem(e,t){if(this.drawWay(e,t))return;let{x:n,y:r,w:i,h:a,kind:o}=t;if(Yi.has(o)||Xi.has(o))return this.drawTerrain(e,t);if(o===`link`)return this.drawLink(e,t);if(o===`key`)return $i(e,n+.5,r+.5,qi(this.lookOf(t)));if(o===`ballista`)return j(e,n+i/2,r+a/2,this.lookOf(t));let s=ha(o),c=.08;if(this.underground&&o===`pillar`)return M().stoneColumn(e,t,this.light,this.columnTones());let l=this.sharedKind(o);if(l){let n={...t,kind:l};if(M().drawFurniture(e,n,this.light,this.lookOf(t),ee.has(l)?this.backSide(n):null,this.ringFit(t)))return}let u=this.wallAnchor(t);if((this.underground||!l)&&M().drawUnderProp(e,t,u?.light??this.light,this.lookOf(t),u?[0,-1]:X.has(o)?this.propWall(t):Ii.has(o)?this.backWall(t):zi.has(o)?this.runDir(t):[0,0],this.railLinks(t),!!u,o===`niche`&&!u?this.rockBehind(t):2,o===`grille`?this.runOf(t):void 0))return;if(o===`sprite`){e.rect(n+.1,r+.1,i-.2,a-.2).fill({color:7888966,alpha:.5}).stroke({width:.05,color:N});return}if(this.drawUnderItem(e,t))return;if(o===`link_down`){e.rect(n+.06,r+.06,.88,.88).fill(1972758).stroke({width:.06,color:N});for(let t=0;t<4;t++)e.rect(n+.1+t*.2,r+.1,.16,.8).fill(7300955-t*789516);e.moveTo(n+.5,r+.25).lineTo(n+.5,r+.75).moveTo(n+.35,r+.6).lineTo(n+.5,r+.75).lineTo(n+.65,r+.6).stroke({width:.08,color:15918792});return}if(o===`table`&&i===1&&a===1){e.circle(n+.5,r+.5,.36).fill(9068600).stroke({width:.06,color:N}),e.circle(n+.5,r+.5,.24).stroke({width:.03,color:N,alpha:.4});return}if(o===`spiral_stair`){let[t,i]=[n+.5,r+.5];e.circle(t,i,.45).fill(8022616).stroke({width:.06,color:N});for(let n=0;n<8;n++){let r=n/8*Math.PI*2;e.moveTo(t,i).lineTo(t+Math.cos(r)*.45,i+Math.sin(r)*.45)}e.stroke({width:.03,color:N,alpha:.7}),e.circle(t,i,.08).fill(N);return}if(o===`bucket`){e.circle(n+.5,r+.5,.18).fill(6179382).stroke({width:.04,color:N});return}if(o===`rack`){e.rect(n+.08,r+.08,i-.16,a-.16).fill(6964264).stroke({width:.06,color:N}),e.moveTo(n+.25,r+a/2).lineTo(n+i-.25,r+a/2).stroke({width:.05,color:4146248});return}if(o===`chair`){e.circle(n+.5,r+.5,.2).fill(7227948).stroke({width:.05,color:N});return}if(o===`booth_seat`){e.roundRect(n+.1,r+.1,.8,.8,.12).fill(8010542).stroke({width:.05,color:N}),e.roundRect(n+.22,r+.22,.56,.56,.08).fill(9390650);return}if(o===`booth_table`){e.rect(n+.08,r+.08,.84,.84).fill(6964264).stroke({width:.06,color:N}),e.rect(n+.16,r+.16,.68,.68).fill(9068600);return}let[d,f,p,m]=[n+c,r+c,n+i-c,r+a-c];if([`barrel`,`statue`,`pillar`,`cauldron`,`anvil`,`vat`].includes(o)){let t=Math.min(i,a)/2-c;e.circle(n+i/2,r+a/2,t).fill(s).stroke({width:.06,color:N}),o===`barrel`&&e.circle(n+i/2,r+a/2,t*.6).stroke({width:.04,color:N,alpha:.6}),o===`vat`&&e.circle(n+i/2,r+a/2,t*.8).fill(10119722).stroke({width:.05,color:N,alpha:.6});return}if(o===`rug`){e.rect(d,f,p-d,m-f).fill({color:s,alpha:.85}).stroke({width:.06,color:14267242}),e.rect(d+.2,f+.2,p-d-.4,m-f-.4).stroke({width:.03,color:14267242,alpha:.8});return}e.rect(d,f,p-d,m-f).fill(s).stroke({width:.06,color:N});let h=i>=a;switch(o){case`bed`:case`cot`:{let[t,n,r,i]=h?[d+.05,f+.05,.35,m-f-.1]:[d+.05,f+.05,p-d-.1,.35];e.rect(t,n,r,i).fill(15261900);let[a,s]=h?[d+.5,f]:[d,f+.5];e.rect(a,s,p-a,m-s).fill(o===`bed`?7045018:9077360);break}case`crate`:e.moveTo(d,f).lineTo(p,m).moveTo(p,f).lineTo(d,m).stroke({width:.05,color:N,alpha:.6});break;case`shelf`:case`bookcase`:{let t=Math.max(i,a)*3;for(let n=1;n<t;n++){let r=n/t;h?e.moveTo(d+r*(p-d),f).lineTo(d+r*(p-d),m):e.moveTo(d,f+r*(m-f)).lineTo(p,f+r*(m-f))}e.stroke({width:.05,color:o===`bookcase`?11553338:9071173});break}case`hearth`:case`forge`:case`oven`:e.rect((d+p)/2-.25,(f+m)/2-.2,.5,.4).fill(14710826);break;case`trapdoor`:e.moveTo(d,(f+m)/2).lineTo(p,(f+m)/2).stroke({width:.05,color:N});break;case`cage`:for(let t=1;t<i*3;t++)e.moveTo(d+t/(i*3)*(p-d),f).lineTo(d+t/(i*3)*(p-d),m);e.stroke({width:.04,color:10131086});break;case`table`:case`long_table`:case`desk`:case`display`:e.rect(d+.1,f+.1,p-d-.2,m-f-.2).fill(9068600);break;case`altar`:e.rect(d+.15,f+.15,p-d-.3,m-f-.3).fill(14208957);break;case`bath`:e.rect(d+.15,f+.15,p-d-.3,m-f-.3).fill(5212840)}}drawUnderItem(e,t){if(!this.underground)return!1;let{x:n,y:r,w:i,h:a,kind:o}=t,[s,c]=[n+i/2,r+a/2],[l,u]=this.light,[d,f]=[-l*.14,-u*.14],p=(e,t)=>{let n=n=>Math.min(255,Math.max(0,Math.round((e>>n&255)*t)));return n(16)<<16|n(8)<<8|n(0)},m=(t,n,r,i=1)=>e.ellipse(t+d*i,n+f*i,r*1.08,r*.96).fill({color:0,alpha:.32}),h=(t,n,r,i,a=1)=>{a>0&&m(t,n,r,a),e.circle(t,n,r).fill(p(i,.72)),e.circle(t+l*r*.16,n+u*r*.16,r*.84).fill(i),e.circle(t+l*r*.4,n+u*r*.4,r*.36).fill({color:p(i,1.35),alpha:.75}),e.circle(t,n,r).stroke({width:.03,color:N,alpha:.65})},g=(t,n,r,i,a,o=1)=>{o>0&&e.rect(t+d*o*1.4,n+f*o*1.4,r,i).fill({color:0,alpha:.32}),e.rect(t,n,r,i).fill(a);let s=Math.min(r,i)*.16;e.rect(l<0?t:t+r-s,n,s,i).fill({color:p(a,1.3),alpha:.6}),e.rect(t,u<0?n:n+i-s,r,s).fill({color:p(a,1.3),alpha:.6}),e.rect(l<0?t+r-s:t,n,s,i).fill({color:p(a,.65),alpha:.6}),e.rect(t,u<0?n+i-s:n,r,s).fill({color:p(a,.65),alpha:.6}),e.rect(t,n,r,i).stroke({width:.03,color:N,alpha:.65})};switch(o){case`sewage`:case`lava`:return!0;case`trapdoor`:return g(n+.12,r+.12,.76,.76,7227948,0),e.moveTo(n+.12,c).lineTo(n+.88,c).stroke({width:.04,color:N,alpha:.6}),e.circle(s,c+.2,.07).stroke({width:.03,color:10265519}),!0;case`winch`:return g(n+.1,r+.2,.8,.6,7227948,1.4),e.ellipse(s,c,.18,.28).fill(4867136).stroke({width:.03,color:N}),!0;case`cot`:{let t=a>=i;return m(s,c,Math.max(i,a)*.32,.6),e.roundRect(n+.15,r+.15,i-.3,a-.3,.15).fill(o===`cot`?8215100:7036229).stroke({width:.03,color:N,alpha:.6}),e.roundRect(n+.2,r+.2,t?i-.4:.35,t?.35:a-.4,.1).fill(14208957),!0}case`chest`:return g(n+.16,r+.24,.68,.52,8017200,1),e.rect(n+.16,c-.04,.68,.08).fill(4146248),e.rect(s-.05,c-.06,.1,.12).fill(14267242),!0;case`sarcophagus`:return g(n+.08,r+.08,i-.16,a-.16,12104358,1.4),h(a>i?s:n+.3,a>i?r+.3:c,.11,13617338,0),e.roundRect(a>i?s-.14:n+.45,a>i?r+.45:c-.14,a>i?.28:i-.65,a>i?a-.65:.28,.08).fill({color:13617338,alpha:.85}),!0;case`statue`:return g(n+.1,r+.1,i-.2,a-.2,9078138,1),h(s,c,.24,12104358,2.2),!0;case`pillar`:return h(s,c,Math.min(i,a)*.42,11051670,2.6),!0;case`bookshelf`:{g(n+.1,r+.1,i-.2,a-.2,6176808,2);let t=a>i;for(let o=0;o<6;o++){let s=(o+.5)/6,c=[8330525,1981023,4153130,8017200][o%4];t?e.rect(n+.2,r+.15+s*(a-.4),i-.4,(a-.4)/6-.03).fill(c):e.rect(n+.15+s*(i-.4),r+.2,(i-.4)/6-.03,a-.4).fill(c)}return!0}case`rug`:return e.rect(n+.1,r+.1,i-.2,a-.2).fill({color:7023146,alpha:.75}).stroke({width:.05,color:11569738,alpha:.7}),e.rect(n+.3,r+.3,i-.6,a-.6).stroke({width:.03,color:11569738,alpha:.5}),!0;case`table`:return g(n+.1,r+.1,i-.2,a-.2,8017200,1.2),!0;case`crate`:return g(n+.12,r+.12,.76,.76,10123854,1.2),e.moveTo(n+.14,r+.14).lineTo(n+.86,r+.86).moveTo(n+.86,r+.14).lineTo(n+.14,r+.86).stroke({width:.04,color:6176808,alpha:.7}),!0;case`barrel`:return h(s,c,.36,8017200,1.2),e.circle(s,c,.24).stroke({width:.035,color:4146248,alpha:.8}),!0;case`weapon_rack`:g(n+.15,r+.3,.7,.4,6176808,1.6);for(let t of[.3,.5,.7])e.moveTo(n+t,r+.18).lineTo(n+t,r+.82);return e.stroke({width:.035,color:10265519}),!0;case`cage`:m(s,c,Math.min(i,a)*.45,1.6),e.rect(n+.1,r+.1,i-.2,a-.2).fill({color:1972758,alpha:.35}).stroke({width:.06,color:5591629});for(let t=1;t<i*3;t++)e.moveTo(n+.1+t/(i*3)*(i-.2),r+.1).lineTo(n+.1+t/(i*3)*(i-.2),r+a-.1);return e.stroke({width:.035,color:9078398}),!0;case`rack`:return g(n+.15,r+.1,i-.3,a-.2,7227948,1),e.circle(s,r+.25,.08).circle(s,r+a-.25,.08).fill(4146248),!0;case`altar`:return g(n+.08,r+.12,i-.16,a-.24,12893358,1.6),e.rect(s-.2,c-.08,.4,.16).fill({color:8330525,alpha:.7}),!0;default:return!1}}levelInfo(){return this.interior.levels.map((e,t)=>({name:this.levelName(t),z:e.z,...this.levelWay(e)}))}static siteName(t){return e.names[t]??e.ways[t]}wayItems=null;isWay(e){return this.wayScale(e)>0}wayScale(e){if(!this.wayItems){this.wayItems=new Map;let{nx:e,ny:t}=this.interior,n=this.underground?`b:`:`u:`;for(let r of this.interior.levels)for(let i of r.links??[]){if(!i.to.startsWith(n)||i.far)continue;let a=r.furniture.find(e=>e.x===i.x&&e.y===i.y&&e.w===1&&e.h===1);if(!a)continue;let o=r.cells[a.y*e+a.x],s=!0;for(let n=a.y-1;n<=a.y+1;n++)for(let i=a.x-1;i<=a.x+1;i++)s&&=i>=0&&n>=0&&i<e&&n<t&&r.cells[n*e+i]===o;this.wayItems.set(a,s?qa:Ja)}}return this.wayItems.get(e)??0}linkText(t){let n=t.far,r=t.to===this.interior.id?this.levelName(n.level):e.names[t.to]??n.name;return`${Va(n.label||n.way)} to ${r}`}linkUp(e){let t=this.interior.levels.findIndex(t=>t.furniture.includes(e)),n=this.interior.levels[t]?.links?.find(t=>t.far&&t.x===e.x&&t.y===e.y);return n?.far?n.to.startsWith(`b:`)||n.to===this.interior.id&&n.far.level>t:!1}drawLink(e,t){let n=xe[t.variant??0]??`stairs`,[r,i]=[t.x+.5,t.y+.5],a={...t,w:1,h:1};n===`ladder`?M().drawUnderProp(e,{...a,kind:`ladder`},this.light,0,[0,-1]):n===`passage`?M().drawUnderProp(e,{...a,kind:`tunnel`},this.light,0,[0,-1]):n===`trapdoor`?M().drawFurniture(e,{...a,kind:`trapdoor`},this.light,0):n===`rope`?(e.circle(r,i,.36).fill(1972758).stroke({width:.06,color:N}),e.circle(r+.18,i-.18,.17).fill(11831890).stroke({width:.05,color:N}),e.circle(r+.18,i-.18,.09).stroke({width:.04,color:8018482}),e.moveTo(r+.06,i-.1).quadraticCurveTo(r-.05,i,r-.02,i+.2).stroke({width:.07,color:11831890,cap:`round`})):n===`portal`?(e.circle(r,i,.42).fill({color:7163844,alpha:.3}).stroke({width:.06,color:N}),e.circle(r,i,.31).stroke({width:.08,color:12165887}),e.circle(r,i,.15).fill({color:15656703,alpha:.85})):M().drawUnderProp(e,{...a,kind:this.linkUp(t)?`up`:`down`},this.light,0,[0,-1])}wayLink(e,t){return e.links?.find(e=>e.x===t.x&&e.y===t.y)}wayNear(e,t,n){return e.furniture.find(e=>Math.abs(t-e.x-.5)<this.wayScale(e)/2&&Math.abs(n-e.y-.5)<this.wayScale(e)/2)}drawingWay=!1;drawWay(e,t){if(this.drawingWay||!this.isWay(t))return!1;let[n,r]=[t.x+.5,t.y+.5];this.drawingWay=!0;let[i,a]=[this.wayTurn(),this.wayScale(t)],[o,s]=[[1,0],[0,1],[-1,0],[0,-1]][i],[c,l,u,d]=[a*o,a*s,-a*s,a*o];e.save().setTransform(c,l,u,d,n-(c*n+u*r),r-(l*n+d*r));try{t.kind===`link_down`&&!this.underground?M().drawUnderProp(e,{...t,kind:`down`},this.light,this.lookOf(t),[0,-1]):t.kind===`exit`&&this.underground?M().drawUnderProp(e,{...t,kind:`up`},this.light,this.lookOf(t),[0,-1]):this.drawItem(e,t),this.wayArrow(e,t)}finally{e.restore(),this.drawingWay=!1}return!0}wayArrow(e,t){let n=t.kind===`link_down`||t.kind===`down`;if(!n&&t.kind!==`exit`&&t.kind!==`up`)return;let{x:r,y:i}=t,[a,o,s,c]=n?[.26,.66,.54,.68]:[.72,.3,.42,.28];for(let[t,n]of[[.096,N],[.06,Ya]])e.moveTo(r+.5,i+a).lineTo(r+.5,i+o).moveTo(r+.38,i+s).lineTo(r+.5,i+c).lineTo(r+.62,i+s),e.stroke({width:t,color:n,cap:`round`,join:`round`})}waySpots=[];wayMarks=[];wayLabels(t){this.waySpots=[],this.wayMarks=[];let n=new V({fontFamily:[`Palatino Linotype`,`Georgia`,`serif`],fontSize:12,fontWeight:`bold`,fill:`#fde68a`,stroke:{color:`#2a1f14`,width:3,join:`round`}}),r=(e,t)=>{let r=new wt({text:e,style:n,anchor:.5,resolution:2});this.labels.addChild(r),this.waySpots.push({text:r,rect:t})};for(let n of t.furniture){if(!this.isWay(n))continue;let i=this.wayLink(t,n),a=i&&e.siteName(i.to),o=this.wayScale(n)/2;r(a?`${this.underground?`Up`:`Down`} to ${a}`:Va(n.name),[n.x+.5-o,n.y+.5-o,n.x+.5+o,n.y+.5+o])}let i=this.waySpots.length;for(let e of t.links??[])e.far&&r(this.linkText(e),[e.x,e.y,e.x+1,e.y+1]);for(let e of t.links??[])this.waySpots.some(t=>t.rect[0]<e.x+.5&&e.x+.5<t.rect[2]&&t.rect[1]<e.y+.5&&e.y+.5<t.rect[3])||this.wayMarks.push([e.x,e.y,e.x+1,e.y+1]);let[a,o,s,c]=this.interior.stairs;if(!t.has_stairs||this.underground)return;let l=this.interior.levels[this.level+1];i&&t.z<0&&l?.has_stairs?r(l.z===0?`Up to the ground floor`:`Up to ${this.levelName(this.level+1)}`,[a,o,a+s,o+c]):this.wayMarks.push([a,o,a+s,o+c])}placeWays(e,t){let[n,r]=[Math.cos(e.turn),Math.sin(e.turn)],i=(e,t)=>[e*n-t*r,e*r+t*n],a=(e,t)=>[e*n+t*r,-e*r+t*n],o=(e,t,n,r)=>[e-n,t-r,e+n,t+r],s=(e,t)=>e[0]<t[2]&&t[0]<e[2]&&e[1]<t[3]&&t[1]<e[3],c=(e,t)=>Math.hypot(Math.max(0,t[0]-e[2],e[0]-t[2]),Math.max(0,t[1]-e[3],e[1]-t[3])),l=t=>{let n=[[t[0],t[1]],[t[2],t[1]],[t[0],t[3]],[t[2],t[3]]].map(([t,n])=>i(...e.worldToScreen(...this.toWorld(t,n))));return[Math.min(...n.map(e=>e[0])),Math.min(...n.map(e=>e[1])),Math.max(...n.map(e=>e[0])),Math.max(...n.map(e=>e[1]))]},u=this.waySpots.map(e=>l(e.rect)),d=[...u,...this.wayMarks.map(l)],f=[];this.waySpots.forEach((n,r)=>{let l=n.text;if(l.rotation=-e.turn,l.visible=t>16,!l.visible)return;let p=u[r],m=d.filter((e,t)=>t!==r),[h,g,_]=[l.width/2,l.height/2,3],[v,y]=[(p[0]+p[2])/2,(p[1]+p[3])/2],b=[[v,p[3]+_+g],[v,p[1]-_-g],[p[2]+_+h,y],[p[0]-_-h,y]],x=b.map(([e,t],n)=>{let r=o(e,t,h,g),i=c([e,t,e,t],p),a=Math.min(1/0,...m.map(n=>c([e,t,e,t],n))),s=Math.min(1/0,...[...m,...f].map(e=>c(r,e)));return{x:e,y:t,side:n,lb:r,ok:a>i&&s>0,room:s,lead:a-i}}),S=x.filter(e=>e.ok),C=S.find(e=>e.room>=l.height)??S.sort((e,t)=>t.room-e.room)[0]??x.sort((e,t)=>t.lead-e.lead)[0];l.position.set(...a(C.x,C.y)),f.push(C.lb);let[w,T]=b[C.side^1];for(let e of this.labelSpots){let t=e.text;if(!t.visible)continue;let[n,r]=i(t.x,t.y);if(![p,C.lb].some(e=>s(e,o(n,r,t.width/2+4,t.height/2+2))))continue;let[c,l]=C.side<2?[w,T+(C.side===0?-1:1)*(t.height/2-g)]:[w+(C.side===2?-1:1)*(t.width/2-h),T];t.position.set(...a(c,l))}})}wayTurn(){let e=this.interior;return(Math.round(Math.atan2(-e.axis[1],e.across[1])/(Math.PI/2))%4+4)%4}levelWay(t){let n=this.underground?`b:`:`u:`,r=t.links?.find(t=>t.to.startsWith(n)&&e.siteName(t.to));return r?{way:`${this.underground?`up`:`down`} to ${e.siteName(r.to)}`}:{}}levelName(t){return e.names[`l:${this.interior.id}:${t}`]??this.interior.levels[t].name}roomNameAt(e,t){let n=this.roomAt(e,t);return n?this.roomName(this.level,Number(n.slice(n.lastIndexOf(`:`)+1))):void 0}roomName(t,n){return e.names[`r:${this.interior.id}:${t}:${n}`]??this.interior.levels[t].rooms[n].kind}};export{qt as A,Ut as C,Lt as D,Wt as E,wt as F,V as I,B as L,Yt as M,Vt as N,Jt as O,Et as P,ke as R,Rt as S,H as T,fn as _,X as a,an as b,Wi as c,mr as d,Ln as f,pn as g,hn as h,Ii as i,Xt as j,Kt as k,na as l,gn as m,Ga as n,Gi as o,U as p,Ba as r,qi as s,$a as t,Yr as u,rn as v,Pt as w,Zt as x,nn as y};