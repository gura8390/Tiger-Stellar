"""Reference and list checks; does not validate rendering or mesh interiors."""
import hashlib,re,struct
from pathlib import Path,PurePosixPath

class AssetsNames:
    def asset_reference(self,value,n):
        if not value:return
        rel=value.replace('\\','/');parts=PurePosixPath(rel)
        if parts.is_absolute() or '..' in parts.parts or ':' in rel:self.emit('asset-path','resource path must stay within a loaded game root: '+value,n);return
        if any(x in rel for x in ['$','[','@']):self.gap('dynamic resource path: '+value,n);return
        # Native .asset/.gfx references may be relative to their source file.
        # Overlay search always uses the relative candidate across ALL layers,
        # never an absolute sibling bypassing replace_path or dependency order.
        candidates=[rel];source=Path(n.get('_source_path') or self.file)
        source_rel=None
        for root in self.asset_roots:
            if source.is_relative_to(root):source_rel=source.relative_to(root).as_posix();break
        if source_rel:
            candidates.append((PurePosixPath(source_rel).parent/parts).as_posix())
            # Vanilla mesh declarations share placeholder material textures.
            if source_rel.startswith('gfx/models/') and '/' not in rel and parts.suffix.lower() in {'.dds','.tga'}:candidates.append('gfx/models/'+rel)
        found=None;resolved=None
        for candidate in dict.fromkeys(candidates):
            for root in reversed(self.asset_roots):
                p=root/candidate
                if p.is_file():found=p;resolved=candidate;break
                if any(candidate.lower().startswith(prefix) for prefix in getattr(self,'asset_replace_paths',{}).get(str(root),[])):break
            if found:break
        if found is None:self.emit('missing-asset','resource file not found: '+rel,n);return
        cache=getattr(self,'_asset_checks',None)
        if cache is None:cache={};self._asset_checks=cache
        stat=found.stat();stamp=(stat.st_size,stat.st_mtime_ns)
        if str(found) not in cache or cache[str(found)][0]!=stamp:
            data=found.read_bytes();self.asset_inputs[str(found)]={'relative_path':resolved,'path':str(found),'sha256':hashlib.sha256(data).hexdigest(),'asset':True}
            good=True
            if found.suffix.lower()=='.dds':
                good=len(data)>=128 and data[:4]==b'DDS '
                if good:
                    size,flags,height,width=struct.unpack_from('<4I',data,4)
                    good=size==124 and height>0 and width>0 and struct.unpack_from('<I',data,76)[0]==32
                    if data[84:88]==b'DX10' and len(data)<148:good=False
            cache[str(found)]=(stamp,good)
        if not cache[str(found)][1]:self.emit('dds-header','invalid or truncated DDS header/dimensions: '+rel,n)
        self.checked['resource_file_references']+=1

    def index_graphics(self,ast,path):
        from stellar import children,fields
        def walk(nodes):
            for n in nodes:
                kind=n.get('key','').lower()
                if kind in {'spritetype','pdxmesh','entity'}:
                    name=fields(n).get('name',{}).get('value')
                    if name:self.entities['gfx_'+kind].add(name)
                walk(children(n))
        walk(ast)

    def graphics(self,ast):
        from stellar import children,fields
        paths={'texturefile','texture','file','diffuse','normal','specular','texture_diffuse','texture_normal','texture_specular','diffuse_texture','normal_texture','specular_texture'}
        def walk(nodes,parent=''):
            for n in nodes:
                kind=n.get('key','').lower();value=n.get('value')
                if kind in paths and value and ('/' in value or '\\' in value or value.lower().endswith(('.dds','.tga','.mesh','.anim'))):self.asset_reference(value,n)
                elif kind in {'pdxmesh','clone','entity'} and value:
                    category='gfx_pdxmesh' if kind=='pdxmesh' else 'gfx_entity'
                    self.ref(category,self.scalar(n),n)
                if 'children' in n:walk(children(n),kind)
        walk(ast);self.checked['graphics_reference_files']+=1
        # Known reference checks are complete; shader/state/GUI evaluation is not.
        self.gap('graphics behavior/shader/material semantics beyond resource references are not implemented')

    def name_list(self,n):
        from stellar import children,Context
        def string_list(node):
            if not self.require_block(node):return
            for c in children(node):
                if 'key' in c or 'children' in c:self.emit('name-list-structure','name list expects scalar name/localization entries',c)
                else:self.localisation(c)
        def pool(node):
            if not self.require_block(node):return
            # Simple ship-name pools are scalar lists. Planet/army pools use
            # typed alternatives; unsupported alternatives remain explicit gaps.
            if all('key' not in c for c in children(node)):string_list(node);return
            for c in children(node):
                k=c.get('key')
                if k in {'names','random_names'}:string_list(c)
                elif k in {'sequential_name','sequential_name_male','sequential_name_female'}:self.localisation(c)
                elif k=='weight':self.numeric(c)
                elif k=='generic' or k in self.entities['planet_classes']:pool(c)
                else:self.gap('name list pool alternative: '+str(k),c)
        allowed={'category','alias','randomized','customize_random_override','ship_names','ship_class_names','fleet_names','army_names','planet_names','character_names','selectable','by_rarity_names','design_names','trigger','should_name_home_system_planets'}
        for c in children(n):
            k=c.get('key')
            if k not in allowed:self.emit('unknown-field','unknown name list field '+str(k),c)
            elif k in {'selectable','trigger'}:
                scope='none' if k=='selectable' else 'country'
                if self.require_block(c):self.script(children(c),'triggers',Context(scope,scope))
            elif k=='design_names':string_list(c)
            elif k=='by_rarity_names':
                if self.require_block(c):
                    for rarity in children(c):
                        if rarity.get('key') not in {'common','rare','epic','exceptional'}:self.emit('unknown-field','unknown ship name rarity '+str(rarity.get('key')),rarity)
                        else:string_list(rarity)
            elif k in {'randomized','should_name_home_system_planets'}:self.boolean(c)
            elif k in {'category','alias','customize_random_override'}:self.scalar(c)
            elif k=='fleet_names':pool(c)
            elif k=='character_names':
                if not self.require_block(c):continue
                for variant in children(c):
                    if not self.require_block(variant):continue
                    for names in children(variant):
                        name=names.get('key','')
                        if name in {'full_names_male','full_names_female','first_names_male','first_names_female','second_names','regnal_first_names_male','regnal_first_names_female','regnal_full_names_male','regnal_full_names_female','regnal_second_names'}:string_list(names)
                        else:self.gap('character name list field: '+name,names)
            elif self.require_block(c):
                for sub in children(c):pool(sub)
        self.checked['name_list_definitions']+=1

    def selected_localisation(self,layers):
        from stellar import parse_text
        effective={};self.localisation_definitions={};self.loc_languages={};self.localisation_duplicates=[]
        for layer,root in enumerate(layers):
            for p in sorted((root/'localisation').rglob('*.yml')):effective[p.relative_to(root).as_posix().lower()]=(p,layer)
        entries={}
        for rel,(p,layer) in sorted(effective.items()):
            text=p.read_text(encoding='utf-8-sig',errors='replace');header=re.search(r'^\s*l_(\w+)\s*:',text,re.M)
            if not header:continue
            lang=header[1];seen=set()
            for line,row in enumerate(text.splitlines(),1):
                m=re.match(r'^\s*([^#\s:]+):(?:\d+)?\s*"',row)
                if not m:continue
                key=m[1];record={'path':str(p),'line':line,'layer':layer}
                if (lang,key) in entries:self.localisation_duplicates.append({'language':lang,'key':key,'previous':entries[lang,key],'winner':record,'same_file':key in seen})
                entries[lang,key]=record;seen.add(key)
        for (lang,key),record in entries.items():self.loc_languages.setdefault(lang,set()).add(key)
        self.loc=set().union(*self.loc_languages.values()) if self.loc_languages else set()
        chosen=getattr(self,'language',None)
        for duplicate in self.localisation_duplicates:
            d=duplicate['winner']
            if chosen==duplicate['language'] and d['layer']==len(layers)-1 and len(layers)>1 and (duplicate['same_file'] or duplicate['previous']['layer']==d['layer']):self.emit('duplicate-localization','duplicate '+chosen+' localization key '+duplicate['key'],{'line':d['line']},'warning',d['path'])
