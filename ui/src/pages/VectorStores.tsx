import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { vectorStoresApi, modelsApi, getErrorMessage, type VectorCollection } from '../lib/api'
import {
  Database, Plus, Trash2, X, Check, RotateCcw, AlertTriangle, CheckCircle, Search, FileText
} from 'lucide-react'

interface CollectionFormState {
  name: string
  vector_size: number
  distance: string
}

const DEFAULT_FORM: CollectionFormState = {
  name: '',
  vector_size: 768,
  distance: 'Cosine',
}

export default function VectorStores() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [deleting, setDeleting] = useState<VectorCollection | null>(null)
  const [selectedCol, setSelectedCol] = useState<VectorCollection | null>(null)
  const [activeTab, setActiveTab] = useState<'search' | 'index'>('search')
  const [mutationError, setMutationError] = useState<string | null>(null)

  // Document indexing form state
  const [docText, setDocText] = useState('')
  const [docModel, setDocModel] = useState('')
  const [docPayload, setDocPayload] = useState('{\n  "source": "dashboard_upload"\n}')
  const [indexSuccess, setIndexSuccess] = useState<string | null>(null)
  const [indexError, setIndexError] = useState<string | null>(null)

  // Search form state
  const [searchQuery, setSearchQuery] = useState('')
  const [searchModel, setSearchModel] = useState('')
  const [searchLimit, setSearchLimit] = useState(5)
  interface VectorSearchResult {
    id: string | number
    score: number
    payload?: { content?: string; text?: string; [key: string]: unknown }
  }
  const [searchResults, setSearchResults] = useState<VectorSearchResult[] | null>(null)
  const [searchLoading, setSearchLoading] = useState(false)
  const [searchError, setSearchError] = useState<string | null>(null)

  // Fetch collections
  const { data: collectionsData, isLoading: isCollectionsLoading, error: collectionsError } = useQuery({
    queryKey: ['vector-collections'],
    queryFn: vectorStoresApi.getAll,
  })

  // Fetch models to filter embedding models
  const { data: modelsData } = useQuery({
    queryKey: ['models'],
    queryFn: () => modelsApi.getAll(),
  })

  const embeddingModels = modelsData?.data
    .filter(m => m.pylos?.supports_embeddings)
    .map(m => m.id) || []

  // Ensure default model is selected if available
  const defaultEmbeddingModel = embeddingModels[0] || 'nomic-embed-text'

  const invalidate = () => qc.invalidateQueries({ queryKey: ['vector-collections'] })

  // Collection Mutations
  const createMut = useMutation({
    mutationFn: (form: CollectionFormState) => vectorStoresApi.create(form),
    onSuccess: () => {
      invalidate()
      setShowCreate(false)
      setMutationError(null)
    },
    onError: (e: Error) => setMutationError(e.message),
  })

  const deleteMut = useMutation({
    mutationFn: (name: string) => vectorStoresApi.remove(name),
    onSuccess: () => {
      invalidate()
      setDeleting(null)
      if (selectedCol?.name === deleting?.name) {
        setSelectedCol(null)
      }
    },
  })

  // Document Indexing Mutation
  const indexDocMut = useMutation({
    mutationFn: async () => {
      if (!selectedCol) return
      let parsedPayload: Record<string, unknown>
      try {
        parsedPayload = JSON.parse(docPayload) as Record<string, unknown>
      } catch {
        throw new Error('Invalid JSON format in payload')
      }
      return vectorStoresApi.addDocument(selectedCol.name, {
        text: docText,
        embedding_model: docModel || defaultEmbeddingModel,
        payload: parsedPayload,
      })
    },
    onSuccess: () => {
      setIndexSuccess('Document indexed successfully!')
      setIndexError(null)
      setDocText('')
      invalidate()
      setTimeout(() => setIndexSuccess(null), 3000)
    },
    onError: (e: Error) => {
      setIndexError(e.message)
      setIndexSuccess(null)
    },
  })

  // Vector Search Handler
  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!selectedCol || !searchQuery.trim()) return

    setSearchLoading(true)
    setSearchError(null)
    setSearchResults(null)

    try {
      const results = await vectorStoresApi.search(selectedCol.name, {
        query: searchQuery,
        embedding_model: searchModel || defaultEmbeddingModel,
        limit: searchLimit,
      })
      setSearchResults(results)
    } catch (e: unknown) {
      setSearchError(getErrorMessage(e, 'Search failed'))
    } finally {
      setSearchLoading(false)
    }
  }

  const handleSelectCollection = (col: VectorCollection) => {
    setSelectedCol(col)
    setSearchResults(null)
    setSearchQuery('')
    setDocText('')
    setIndexSuccess(null)
    setIndexError(null)
    setSearchError(null)
    if (!docModel && defaultEmbeddingModel) setDocModel(defaultEmbeddingModel)
    if (!searchModel && defaultEmbeddingModel) setSearchModel(defaultEmbeddingModel)
  }

  return (
    <div className="flex h-full bg-zinc-950 text-zinc-100 overflow-hidden">
      {/* Sidebar: Collections List */}
      <div className="w-80 border-r border-zinc-800 flex flex-col bg-zinc-900/20 shrink-0">
        <div className="p-4 border-b border-zinc-800 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Database className="w-4 h-4 text-emerald-400" />
            <span className="font-semibold text-sm">Collections</span>
          </div>
          <button
            onClick={() => {
              setMutationError(null)
              setShowCreate(true)
            }}
            className="p-1 hover:bg-zinc-800 text-zinc-400 hover:text-emerald-400 rounded transition-colors"
            title="Create Collection"
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {isCollectionsLoading ? (
            <div className="p-4 text-center text-xs text-zinc-500 flex items-center justify-center gap-2">
              <RotateCcw className="w-3 h-3 animate-spin" /> Loading collections...
            </div>
          ) : collectionsError ? (
            <div className="p-4 text-xs text-red-400 bg-red-900/10 rounded-lg border border-red-900/30 m-2 flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
              <span>Failed to fetch collections. Ensure Qdrant is running.</span>
            </div>
          ) : collectionsData?.collections.length === 0 ? (
            <div className="p-6 text-center text-xs text-zinc-500">
              No collections found in Qdrant
            </div>
          ) : (
            collectionsData?.collections.map(col => (
              <div
                key={col.name}
                onClick={() => handleSelectCollection(col)}
                className={`w-full text-left p-3 rounded-lg cursor-pointer transition-all border group relative ${
                  selectedCol?.name === col.name
                    ? 'bg-emerald-500/10 border-emerald-500/30 text-white'
                    : 'border-transparent hover:bg-zinc-800/40 text-zinc-400 hover:text-zinc-200'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="font-medium text-sm truncate pr-8">{col.name}</span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation()
                      setDeleting(col)
                    }}
                    className="opacity-0 group-hover:opacity-100 p-1 hover:bg-red-500/20 text-zinc-500 hover:text-red-400 rounded transition-all absolute right-2 top-2"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
                <div className="flex items-center gap-3 mt-1.5 text-xxs text-zinc-500 font-mono">
                  <span>{col.points_count} points</span>
                  <span>•</span>
                  <span>{col.vector_size}d ({col.distance})</span>
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0">
        {selectedCol ? (
          <div className="flex-1 flex flex-col overflow-hidden">
            {/* Header info */}
            <div className="p-5 border-b border-zinc-800 bg-zinc-900/10 flex items-center justify-between">
              <div>
                <h1 className="text-xl font-semibold text-white flex items-center gap-2">
                  <Database className="w-5 h-5 text-emerald-400" />
                  {selectedCol.name}
                </h1>
                <p className="text-xs text-zinc-400 mt-0.5">
                  Dimensions: <span className="font-mono text-zinc-300">{selectedCol.vector_size}</span> | Distance Metric: <span className="font-mono text-zinc-300">{selectedCol.distance}</span> | Total Documents: <span className="font-mono text-zinc-300">{selectedCol.points_count}</span>
                </p>
              </div>

              {/* Tabs selector */}
              <div className="flex bg-zinc-900 border border-zinc-800 rounded-lg p-0.5">
                <button
                  onClick={() => setActiveTab('search')}
                  className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all flex items-center gap-1.5 ${
                    activeTab === 'search'
                      ? 'bg-zinc-800 text-white shadow'
                      : 'text-zinc-400 hover:text-zinc-200'
                  }`}
                >
                  <Search className="w-3.5 h-3.5" />
                  Query Search
                </button>
                <button
                  onClick={() => setActiveTab('index')}
                  className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all flex items-center gap-1.5 ${
                    activeTab === 'index'
                      ? 'bg-zinc-800 text-white shadow'
                      : 'text-zinc-400 hover:text-zinc-200'
                  }`}
                >
                  <FileText className="w-3.5 h-3.5" />
                  Index Document
                </button>
              </div>
            </div>

            {/* Tab Panels */}
            <div className="flex-1 overflow-y-auto p-6">
              {activeTab === 'search' ? (
                <div className="space-y-6 max-w-4xl">
                  <form onSubmit={handleSearch} className="space-y-4 bg-zinc-900/30 border border-zinc-800/80 rounded-xl p-5">
                    <div className="flex gap-4">
                      <div className="flex-1">
                        <label className="block text-xs text-zinc-400 mb-1.5">Search Query</label>
                        <input
                          type="text"
                          required
                          value={searchQuery}
                          onChange={e => setSearchQuery(e.target.value)}
                          placeholder="Type query to find similar vectors..."
                          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3.5 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                        />
                      </div>
                      <div className="w-56">
                        <label className="block text-xs text-zinc-400 mb-1.5">Embedding Model</label>
                        <select
                          value={searchModel}
                          onChange={e => setSearchModel(e.target.value)}
                          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                        >
                          {embeddingModels.length === 0 ? (
                            <option value={defaultEmbeddingModel}>{defaultEmbeddingModel}</option>
                          ) : (
                            embeddingModels.map(m => (
                              <option key={m} value={m}>{m}</option>
                            ))
                          )}
                        </select>
                      </div>
                      <div className="w-24">
                        <label className="block text-xs text-zinc-400 mb-1.5">Max Results</label>
                        <input
                          type="number"
                          min="1"
                          max="100"
                          value={searchLimit}
                          onChange={e => setSearchLimit(Number(e.target.value))}
                          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 text-center"
                        />
                      </div>
                    </div>

                    <div className="flex justify-end pt-2">
                      <button
                        type="submit"
                        disabled={searchLoading || !searchQuery.trim()}
                        className="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-sm rounded-lg flex items-center gap-2 transition-all disabled:opacity-50"
                      >
                        {searchLoading ? (
                          <RotateCcw className="w-4 h-4 animate-spin" />
                        ) : (
                          <Search className="w-4 h-4" />
                        )}
                        Search
                      </button>
                    </div>
                  </form>

                  {searchError && (
                    <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/10 border border-red-900/30 rounded-lg p-3">
                      <AlertTriangle size={15} />
                      {searchError}
                    </div>
                  )}

                  {/* Search Results Display */}
                  {searchResults !== null && (
                    <div className="space-y-4">
                      <h3 className="text-sm font-semibold text-zinc-300">
                        Search Results ({searchResults.length})
                      </h3>
                      {searchResults.length === 0 ? (
                        <div className="text-center py-12 border border-dashed border-zinc-800 rounded-xl text-zinc-500 text-sm">
                          No matching records found
                        </div>
                      ) : (
                        <div className="space-y-3">
                          {searchResults.map((res, index) => (
                            <div key={res.id || index} className="bg-zinc-900/40 border border-zinc-800/60 rounded-xl p-4 space-y-3">
                              <div className="flex items-center justify-between border-b border-zinc-800/40 pb-2">
                                <span className="text-xs text-zinc-500 font-mono">ID: {res.id}</span>
                                <span className="text-xs bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 px-2 py-0.5 rounded-full font-medium">
                                  Score: {res.score.toFixed(4)}
                                </span>
                              </div>
                              <div className="text-sm text-zinc-300 leading-relaxed">
                                {res.payload?.content || res.payload?.text || (
                                  <span className="text-zinc-500 italic">No content field mapped</span>
                                )}
                              </div>
                              {Object.keys(res.payload || {}).length > 1 && (
                                <div className="space-y-1.5 pt-1">
                                  <span className="text-xxs text-zinc-500 uppercase tracking-wider font-semibold">Metadata Payload</span>
                                  <pre className="text-xs bg-zinc-950 p-2.5 rounded-lg border border-zinc-800/80 font-mono text-zinc-400 overflow-x-auto max-h-40">
                                    {JSON.stringify(res.payload, null, 2)}
                                  </pre>
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              ) : (
                <div className="space-y-6 max-w-2xl bg-zinc-900/30 border border-zinc-800/80 rounded-xl p-6">
                  <h2 className="text-sm font-semibold text-white mb-4 flex items-center gap-2">
                    <FileText className="w-4 h-4 text-emerald-400" /> Index a New Document
                  </h2>
                  <div className="space-y-4">
                    <div>
                      <label className="block text-xs text-zinc-400 mb-1.5">Document Content</label>
                      <textarea
                        rows={6}
                        required
                        value={docText}
                        onChange={e => setDocText(e.target.value)}
                        placeholder="Paste or write the text content to be embedded and indexed..."
                        className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 resize-y"
                      />
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <label className="block text-xs text-zinc-400 mb-1.5">Embedding Model</label>
                        <select
                          value={docModel}
                          onChange={e => setDocModel(e.target.value)}
                          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                        >
                          {embeddingModels.length === 0 ? (
                            <option value={defaultEmbeddingModel}>{defaultEmbeddingModel}</option>
                          ) : (
                            embeddingModels.map(m => (
                              <option key={m} value={m}>{m}</option>
                            ))
                          )}
                        </select>
                      </div>

                      <div>
                        <label className="block text-xs text-zinc-400 mb-1.5">Metadata Payload (JSON)</label>
                        <textarea
                          rows={2}
                          value={docPayload}
                          onChange={e => setDocPayload(e.target.value)}
                          placeholder='{"source": "user_input"}'
                          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-1.5 text-xs text-zinc-200 font-mono focus:outline-none focus:border-emerald-500/50 resize-y"
                        />
                      </div>
                    </div>

                    {indexSuccess && (
                      <div className="flex items-center gap-2 text-emerald-400 text-xs bg-emerald-900/10 border border-emerald-900/30 rounded-lg px-3 py-2">
                        <CheckCircle size={14} />
                        {indexSuccess}
                      </div>
                    )}

                    {indexError && (
                      <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/10 border border-red-900/30 rounded-lg px-3 py-2">
                        <AlertTriangle size={14} />
                        {indexError}
                      </div>
                    )}

                    <div className="flex justify-end pt-2">
                      <button
                        onClick={() => indexDocMut.mutate()}
                        disabled={indexDocMut.isPending || !docText.trim()}
                        className="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-sm rounded-lg flex items-center gap-2 transition-all disabled:opacity-50"
                      >
                        {indexDocMut.isPending ? (
                          <RotateCcw className="w-4 h-4 animate-spin" />
                        ) : (
                          <Check className="w-4 h-4" />
                        )}
                        Index Document
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-zinc-400 bg-zinc-950">
            <div className="bg-zinc-900/50 border border-zinc-800/80 p-8 rounded-2xl max-w-sm flex flex-col items-center text-center shadow-xl">
              <Database className="w-12 h-12 mb-4 text-zinc-500" />
              <h2 className="text-base font-semibold text-zinc-200 mb-1.5">No Collection Selected</h2>
              <p className="text-xs text-zinc-500 mb-5 leading-relaxed">
                Select a collection from the sidebar to query vectors or index new documents.
              </p>
              <button
                onClick={() => {
                  setMutationError(null)
                  setShowCreate(true)
                }}
                className="flex items-center gap-2 px-4 py-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-200 hover:text-white text-xs font-medium rounded-lg transition-all"
              >
                <Plus className="w-3.5 h-3.5" />
                Create New Collection
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Create Collection Modal */}
      {showCreate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs">
          <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-md mx-4">
            <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
              <h2 className="text-base font-semibold text-white">Create Qdrant Collection</h2>
              <button
                onClick={() => setShowCreate(false)}
                className="text-zinc-500 hover:text-white transition-colors"
              >
                <X size={18} />
              </button>
            </div>
            <div className="p-5 space-y-4">
              <div>
                <label className="block text-xs text-zinc-400 mb-1.5">Collection Name *</label>
                <input
                  type="text"
                  required
                  placeholder="e.g. documentation_vectors"
                  onChange={e => {
                    const val = e.target.value
                    createMut.reset()
                    DEFAULT_FORM.name = val
                  }}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                />
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs text-zinc-400 mb-1.5">Vector Dimensions *</label>
                  <select
                    defaultValue="768"
                    onChange={e => {
                      DEFAULT_FORM.vector_size = Number(e.target.value)
                    }}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                  >
                    <option value="384">384 (MiniLM / BGE-Small)</option>
                    <option value="768">768 (Nomic Embed / Cohere)</option>
                    <option value="1536">1536 (OpenAI Ada / Text-3)</option>
                    <option value="3072">3072 (OpenAI Large)</option>
                  </select>
                </div>
                <div>
                  <label className="block text-xs text-zinc-400 mb-1.5">Distance Metric *</label>
                  <select
                    defaultValue="Cosine"
                    onChange={e => {
                      DEFAULT_FORM.distance = e.target.value
                    }}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                  >
                    <option value="Cosine">Cosine Similarity</option>
                    <option value="Dot">Dot Product</option>
                    <option value="Euclid">Euclidean Distance</option>
                  </select>
                </div>
              </div>

              {mutationError && (
                <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/15 border border-red-900/30 rounded-lg px-3 py-2">
                  <AlertTriangle size={14} />
                  {mutationError}
                </div>
              )}
            </div>
            <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
              <button
                onClick={() => setShowCreate(false)}
                className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => createMut.mutate(DEFAULT_FORM)}
                disabled={createMut.isPending || !DEFAULT_FORM.name.trim()}
                className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2 transition-all"
              >
                {createMut.isPending ? (
                  <RotateCcw size={14} className="animate-spin" />
                ) : (
                  <Check size={14} />
                )}
                Create
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirmation Modal */}
      {deleting && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs">
          <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
                <AlertTriangle size={16} className="text-red-400" />
              </div>
              <div>
                <div className="font-semibold text-white">Delete Collection</div>
                <div className="text-xs text-zinc-500 font-medium">This action cannot be undone</div>
              </div>
            </div>
            <p className="text-sm text-zinc-400 mb-5">
              Delete collection <span className="text-white font-medium">{deleting.name}</span> and all of its vectors?
            </p>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => setDeleting(null)}
                className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => deleteMut.mutate(deleting.name)}
                disabled={deleteMut.isPending}
                className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2 transition-all"
              >
                {deleteMut.isPending ? (
                  <RotateCcw size={13} className="animate-spin" />
                ) : (
                  <Trash2 size={13} />
                )}
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
