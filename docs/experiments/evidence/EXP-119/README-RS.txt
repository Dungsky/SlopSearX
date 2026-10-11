------------------------------------------------
FedWeb 2014 Evaluation: Resource Selection task
------------------------------------------------

The offical metric in the task was nDCG@20; submitted runs were also evaluated
by  nDCG@10, nP@1, nP@5.

The nDCG@k values are calculated with the trec_eval tool, based on the qrels
file 'resource-qrels.txt'
(e.g., for nDCG@20: "./trec_eval -q -m ndcg_cut.20 <qrelfile> <runfile>"). 
The qrels file contains the graded precision per resource, scaled to a
range between 0 and 1000, based on the UDM relevance level weights for
the individual results [1]:

weights = {'Non':0.0, 'Rel':0.158, 'HRel': 0.546, 'Key':1.0, 'Nav':1.0}

The nP@k metric, normalized precision introduced in the FedWeb 2013 track [2],
represents the graded precision of the selected first k resources, 
normalized by the graded precision of the best possible k resources for
the given topic, irrespective of the order of these k resources.

[1] T. Demeester, R. Aly, D. Hiemstra, D. Nguyen, D. Trieschnigg, and C. Develder. Exploiting User Disagreement for Web Search Evaluation: an Experimental Approach. In 7th ACM International Conference on Web Search and Data Mining (WSDM 2014), pages 33--42, 2014.
  
[2] T. Demeester, D. Trieschnigg, D. Nguyen, and D. Hiemstra. Overview of the TREC 2013 Federated Web Search Track. In The 22nd Text Retrieval Conference (TREC 2013), 2013.
